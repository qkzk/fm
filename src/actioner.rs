use std::path::PathBuf;

use tuikit::prelude::{Event, Key, MouseButton};

use crate::bulkrename::Bulkrename;
use crate::compress::decompress;
use crate::copy_move::CopyMove;
use crate::fileinfo::{FileKind, PathContent, SortBy};
use crate::fm_error::{FmError, FmResult};
use crate::keybindings::Keybindings;
use crate::last_edition::LastEdition;
use crate::mode::{MarkAction, Mode};
use crate::preview::Preview;
use crate::status::Status;
use crate::tab::Tab;
use crate::term_manager::MIN_WIDTH_FOR_DUAL_PANE;

use std::borrow::Borrow;
use std::cmp::min;
use std::fs;
use std::path;

use copypasta::{ClipboardContext, ClipboardProvider};
use log::info;

use crate::content_window::ContentWindow;
use crate::filter::FilterKind;
use crate::opener::execute_in_child;

/// Struct which mutates `tabs.selected()..
/// Holds a mapping which can't be static since it's read from a config file.
/// All keys are mapped to relevent events on tabs.selected().
/// Keybindings are read from `Config`.
pub struct Actioner {
    binds: Keybindings,
}

impl Actioner {
    /// Creates a map of configurable keybindings to `EventChar`
    /// The `EventChar` is then associated to a `tabs.selected(). method.
    pub fn new(binds: Keybindings) -> Self {
        Self { binds }
    }
    /// Reaction to received events.
    pub fn read_event(&self, status: &mut Status, ev: Event) -> FmResult<()> {
        match ev {
            Event::Key(Key::ESC) => Self::escape(status),
            Event::Key(Key::Up) => Self::up(status),
            Event::Key(Key::Down) => Self::down(status),
            Event::Key(Key::Left) => Self::left(status),
            Event::Key(Key::Right) => Self::right(status),
            Event::Key(Key::Backspace) => Self::backspace(status),
            Event::Key(Key::Ctrl('d')) => Self::delete(status),
            Event::Key(Key::Ctrl('q')) => Self::escape(status),
            Event::Key(Key::Char(c)) => self.char(status, c),
            Event::Key(Key::Home) => Self::home(status),
            Event::Key(Key::End) => Self::end(status),
            Event::Key(Key::PageDown) => Self::page_down(status),
            Event::Key(Key::PageUp) => Self::page_up(status),
            Event::Key(Key::Enter) => Self::enter(status),
            Event::Key(Key::Tab) => Self::tab(status),
            Event::Key(Key::BackTab) => Self::backtab(status),
            Event::Key(Key::WheelUp(_, _, _)) => Self::up(status),
            Event::Key(Key::WheelDown(_, _, _)) => Self::down(status),
            Event::Key(Key::SingleClick(MouseButton::Left, row, _)) => {
                Self::left_click(status, row);
                Ok(())
            }
            Event::Key(Key::SingleClick(MouseButton::Right, row, _)) => {
                Self::right_click(status, row);
                Ok(())
            }
            Event::Key(Key::Ctrl('f')) => Self::ctrl_f(status),
            Event::Key(Key::Ctrl('c')) => Self::ctrl_c(status),
            Event::Key(Key::Ctrl('p')) => Self::ctrl_p(status),
            Event::Key(Key::Ctrl('r')) => Self::refresh_selected_view(status),
            Event::Key(Key::Ctrl('x')) => Self::ctrl_x(status),
            Event::User(_) => Self::refresh_selected_view(status),
            Event::Resize { width, height } => Self::resize(status, width, height),
            _ => Ok(()),
        }
    }

    /// Leaving a mode reset the window
    fn escape(status: &mut Status) -> FmResult<()> {
        Self::event_normal(status.selected())
    }

    /// Move one line up
    fn up(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => Self::event_up_one_row(status.selected()),
            Mode::Jump => Self::event_jumplist_prev(status),
            Mode::History => Self::event_history_prev(status.selected()),
            Mode::Shortcut => Self::event_shortcut_prev(status.selected()),
            Mode::Goto | Mode::Exec | Mode::Search => {
                status.selected().completion.prev();
            }
            _ => (),
        };
        Ok(())
    }

    /// Move one line down
    fn down(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => {
                Self::event_down_one_row(status.selected())
            }
            Mode::Jump => Self::event_jumplist_next(status),
            Mode::History => Self::event_history_next(status.selected()),
            Mode::Shortcut => Self::event_shortcut_next(status.selected()),
            Mode::Goto | Mode::Exec | Mode::Search => {
                status.selected().completion.next();
            }
            _ => (),
        };
        Ok(())
    }

    /// Move left in a string, move to parent in normal mode
    fn left(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal => Self::event_move_to_parent(status.selected()),
            Mode::Rename
            | Mode::Chmod
            | Mode::Newdir
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto
            | Mode::RegexMatch
            | Mode::Filter => {
                Self::event_move_cursor_left(status.selected());
                Ok(())
            }

            _ => Ok(()),
        }
    }

    /// Move right in a string, move to children in normal mode.
    fn right(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal => Self::exec_file(status.selected()),
            Mode::Rename
            | Mode::Chmod
            | Mode::Newdir
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto
            | Mode::RegexMatch
            | Mode::Filter => {
                Self::event_move_cursor_right(status.selected());
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Deletes a char in input string
    fn backspace(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Rename
            | Mode::Newdir
            | Mode::Chmod
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto
            | Mode::RegexMatch
            | Mode::Filter => {
                Self::event_delete_char_left(status.selected());
                Ok(())
            }
            Mode::Normal => Ok(()),
            _ => Ok(()),
        }
    }

    /// Deletes chars right of cursor in input string.
    /// Remove current tab in normal mode.
    fn delete(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Rename
            | Mode::Newdir
            | Mode::Chmod
            | Mode::Newfile
            | Mode::Exec
            | Mode::Search
            | Mode::Goto
            | Mode::RegexMatch
            | Mode::Filter => {
                Self::event_delete_chars_right(status.selected());
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Move to top or beggining of line.
    fn home(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => Self::event_go_top(status.selected()),
            _ => Self::event_cursor_home(status.selected()),
        };
        Ok(())
    }

    /// Move to end or end of line.
    fn end(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => Self::event_go_bottom(status.selected()),
            _ => Self::event_cursor_end(status.selected()),
        };
        Ok(())
    }

    /// Move down 10 rows
    fn page_down(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => Self::event_page_down(status.selected()),
            _ => (),
        };
        Ok(())
    }

    /// Move up 10 rows
    fn page_up(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Normal | Mode::Preview | Mode::Help => Self::event_page_up(status.selected()),
            _ => (),
        };
        Ok(())
    }

    /// Execute a command
    fn enter(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Rename => Self::exec_rename(status.selected())?,
            Mode::Newfile => Self::exec_newfile(status.selected())?,
            Mode::Newdir => Self::exec_newdir(status.selected())?,
            Mode::Chmod => Self::exec_chmod(status)?,
            Mode::Exec => Self::exec_exec(status.selected())?,
            Mode::Search => Self::exec_search(status.selected()),
            Mode::Goto => Self::exec_goto(status.selected())?,
            Mode::RegexMatch => Self::exec_regex(status)?,
            Mode::Jump => Self::exec_jump(status)?,
            Mode::History => Self::exec_history(status.selected())?,
            Mode::Shortcut => Self::exec_shortcut(status.selected())?,
            Mode::Filter => Self::exec_filter(status.selected())?,
            Mode::Normal => Self::exec_file(status.selected())?,
            Mode::NeedConfirmation | Mode::Help | Mode::Sort | Mode::Preview | Mode::Marks(_) => (),
        };

        status.selected().input.reset();
        status.selected().mode = Mode::Normal;
        Ok(())
    }

    /// Select this file
    fn left_click(status: &mut Status, row: u16) {
        if let Mode::Normal = status.selected().mode {
            Self::event_select_row(status.selected(), row)
        }
    }

    /// Open a directory or a file
    fn right_click(status: &mut Status, row: u16) {
        if let Mode::Normal = status.selected().mode {
            let _ = Self::event_right_click(status.selected(), row);
        }
    }

    /// Select next completion and insert it
    /// Select next tab
    fn tab(status: &mut Status) -> FmResult<()> {
        match status.selected().mode {
            Mode::Goto | Mode::Exec | Mode::Search => {
                Self::event_replace_input_with_completion(status.selected())
            }
            Mode::Normal => status.next(),
            _ => (),
        };
        Ok(())
    }

    /// Select previous tab
    fn backtab(status: &mut Status) -> FmResult<()> {
        if let Mode::Normal = status.selected().mode {
            status.prev()
        }
        Ok(())
    }

    fn ctrl_f(status: &mut Status) -> FmResult<()> {
        status.create_tabs_from_skim()?;
        Ok(())
    }

    fn ctrl_c(status: &mut Status) -> FmResult<()> {
        if let Mode::Normal = status.selected_non_mut().mode {
            return Self::event_filename_to_clipboard(status.selected());
        }
        Ok(())
    }

    fn ctrl_p(status: &mut Status) -> FmResult<()> {
        if let Mode::Normal = status.selected_non_mut().mode {
            return Self::event_filepath_to_clipboard(status.selected());
        }
        Ok(())
    }

    fn refresh_selected_view(status: &mut Status) -> FmResult<()> {
        status.selected().refresh_view()
    }

    fn ctrl_x(status: &mut Status) -> FmResult<()> {
        Self::event_decompress(status.selected())
    }

    /// Match read key to a relevent event, depending on keybindings.
    /// Keybindings are read from `Config`.
    fn char(&self, status: &mut Status, c: char) -> FmResult<()> {
        match status.selected().mode {
            Mode::Newfile | Mode::Newdir | Mode::Chmod | Mode::Rename | Mode::Filter => {
                Self::event_text_insertion(status.selected(), c);
                Ok(())
            }
            Mode::RegexMatch => {
                Self::event_text_insertion(status.selected(), c);
                status.select_from_regex()?;
                Ok(())
            }
            Mode::Goto | Mode::Exec | Mode::Search => {
                Self::event_text_insert_and_complete(status.selected(), c)
            }
            Mode::Normal => match self.binds.get(&c) {
                Some(event_char) => event_char.match_char(status),
                None => Ok(()),
            },
            Mode::Help | Mode::Preview | Mode::Shortcut => Self::event_normal(status.selected()),
            Mode::Jump => Ok(()),
            Mode::History => Ok(()),
            Mode::NeedConfirmation => {
                if c == 'y' {
                    let _ = Self::exec_last_edition(status);
                }
                Self::event_leave_need_confirmation(status.selected());
                Ok(())
            }
            Mode::Marks(MarkAction::Jump) => Self::exec_marks_jump(status, c),
            Mode::Marks(MarkAction::New) => Self::exec_marks_new(status, c),
            Mode::Sort => {
                Self::event_leave_sort(status.selected(), c);
                Ok(())
            }
        }
    }

    fn resize(status: &mut Status, width: usize, height: usize) -> FmResult<()> {
        if width < MIN_WIDTH_FOR_DUAL_PANE {
            status.select_tab(0)?;
            status.set_dual_pane(false);
        } else {
            status.set_dual_pane(true);
        }
        status.selected().set_height(height);
        Self::refresh_selected_view(status)?;
        Ok(())
    }

    pub fn event_clear_flags(status: &mut Status) -> FmResult<()> {
        status.flagged.clear();
        Ok(())
    }

    pub fn event_flag_all(status: &mut Status) -> FmResult<()> {
        status.tabs[status.index]
            .path_content
            .files
            .iter()
            .for_each(|file| {
                status.flagged.insert(file.path.clone());
            });
        status.reset_statuses()
    }

    pub fn event_reverse_flags(status: &mut Status) -> FmResult<()> {
        // for file in self.selected().path_content.files.iter() {
        //     self.toggle_flag_on_path(file.path.clone())
        // }

        status.tabs[status.index]
            .path_content
            .files
            .iter()
            .for_each(|file| {
                if status.flagged.contains(&file.path.clone()) {
                    status.flagged.remove(&file.path.clone());
                } else {
                    status.flagged.insert(file.path.clone());
                }
            });
        status.reset_statuses()
    }
    pub fn event_toggle_flag(status: &mut Status) -> FmResult<()> {
        let file = status.tabs[status.index]
            .path_content
            .selected_file()
            .ok_or_else(|| FmError::new("No selected file"))?;
        status.toggle_flag_on_path(file.path.clone());
        Self::event_down_one_row(status.selected());
        Ok(())
    }

    pub fn event_jumplist_next(status: &mut Status) {
        if status.jump_index < status.flagged.len() {
            status.jump_index += 1;
        }
    }

    pub fn event_jumplist_prev(status: &mut Status) {
        if status.jump_index > 0 {
            status.jump_index -= 1;
        }
    }

    pub fn event_chmod(status: &mut Status) -> FmResult<()> {
        if status.selected().path_content.files.is_empty() {
            return Ok(());
        }
        status.selected().mode = Mode::Chmod;
        if status.flagged.is_empty() {
            status.flagged.insert(
                status.tabs[status.index]
                    .path_content
                    .selected_file()
                    .unwrap()
                    .path
                    .clone(),
            );
        };
        status.reset_statuses()
    }

    pub fn event_jump(status: &mut Status) {
        if !status.flagged.is_empty() {
            status.jump_index = 0;
            status.selected().mode = Mode::Jump
        }
    }

    pub fn event_marks_new(status: &mut Status) {
        status.selected().mode = Mode::Marks(MarkAction::New)
    }

    pub fn event_marks_jump(status: &mut Status) {
        status.selected().mode = Mode::Marks(MarkAction::Jump)
    }

    pub fn exec_marks_new(status: &mut Status, c: char) -> FmResult<()> {
        let path = status.selected().path_content.path.clone();
        status.marks.new_mark(c, path)?;
        Self::event_normal(status.selected())
    }

    pub fn exec_marks_jump(status: &mut Status, c: char) -> FmResult<()> {
        if let Some(path) = status.marks.get(c) {
            let path = path.to_owned();
            status.selected().history.push(&path);
            status.selected().path_content = PathContent::new(path, status.selected().show_hidden)?;
        };
        Self::event_normal(status.selected())
    }

    /// Creates a symlink of every flagged file to the current directory.
    pub fn event_symlink(status: &mut Status) -> FmResult<()> {
        for oldpath in status.flagged.iter() {
            let newpath = status.tabs[status.index].path_content.path.clone().join(
                oldpath
                    .as_path()
                    .file_name()
                    .ok_or_else(|| FmError::new("File not found"))?,
            );
            std::os::unix::fs::symlink(oldpath, newpath)?;
        }
        status.clear_flags_and_reset_view()
    }

    pub fn event_bulkrename(status: &mut Status) -> FmResult<()> {
        Bulkrename::new(status.filtered_flagged_files())?
            .rename(&status.selected_non_mut().opener)?;
        status.selected().refresh_view()
    }

    fn exec_copy_paste(status: &mut Status) -> FmResult<()> {
        status.cut_or_copy_flagged_files(CopyMove::Copy)
    }

    fn exec_cut_paste(status: &mut Status) -> FmResult<()> {
        status.cut_or_copy_flagged_files(CopyMove::Move)
    }

    fn exec_delete_files(status: &mut Status) -> FmResult<()> {
        for pathbuf in status.flagged.iter() {
            if pathbuf.is_dir() {
                std::fs::remove_dir_all(pathbuf)?;
            } else {
                std::fs::remove_file(pathbuf)?;
            }
        }
        status.clear_flags_and_reset_view()
    }

    pub fn exec_chmod(status: &mut Status) -> FmResult<()> {
        if status.selected().input.string.is_empty() {
            return Ok(());
        }
        let permissions: u32 =
            u32::from_str_radix(&status.selected().input.string, 8).unwrap_or(0_u32);
        if permissions <= Status::MAX_PERMISSIONS {
            for path in status.flagged.iter() {
                Status::set_permissions(path.clone(), permissions)?
            }
            status.flagged.clear()
        }
        status.selected().refresh_view()?;
        status.reset_statuses()
    }
    pub fn _exec_last_edition(status: &mut Status) -> FmResult<()> {
        match status.selected().last_edition {
            LastEdition::Delete => Self::exec_delete_files(status),
            LastEdition::CutPaste => Self::exec_cut_paste(status),
            LastEdition::CopyPaste => Self::exec_copy_paste(status),
            LastEdition::Nothing => Ok(()),
        }
    }

    pub fn exec_jump(status: &mut Status) -> FmResult<()> {
        status.selected().input.string.clear();
        let jump_list: Vec<&PathBuf> = status.flagged.iter().collect();
        let jump_target = jump_list[status.jump_index].clone();
        let target_dir = match jump_target.parent() {
            Some(parent) => parent.to_path_buf(),
            None => jump_target.clone(),
        };
        status.selected().history.push(&target_dir);
        status.selected().path_content =
            PathContent::new(target_dir, status.selected().show_hidden)?;
        if let Some(index) = status.find_jump_target(&jump_target) {
            status.selected().line_index = index;
        } else {
            status.selected().line_index = 0;
        }

        let s_index = status.tabs[status.index].line_index;
        status.tabs[status.index].path_content.select_index(s_index);
        let len = status.tabs[status.index].path_content.files.len();
        status.selected().window.reset(len);
        status.selected().window.scroll_to(s_index);
        Ok(())
    }

    pub fn exec_last_edition(status: &mut Status) -> FmResult<()> {
        Self::_exec_last_edition(status)?;
        status.selected().mode = Mode::Normal;
        status.selected().last_edition = LastEdition::Nothing;
        Ok(())
    }

    pub fn exec_regex(status: &mut Status) -> Result<(), regex::Error> {
        status.select_from_regex()?;
        status.selected().input.reset();
        Ok(())
    }

    pub fn event_normal(tab: &mut Tab) -> FmResult<()> {
        tab.input.reset();
        tab.completion.reset();
        tab.path_content.reset_files()?;
        tab.window.reset(tab.path_content.files.len());
        tab.mode = Mode::Normal;
        tab.preview = Preview::empty();
        Ok(())
    }

    pub fn event_up_one_row(tab: &mut Tab) {
        match tab.mode {
            Mode::Normal => {
                tab.path_content.select_prev();
                if tab.line_index > 0 {
                    tab.line_index -= 1;
                }
            }
            Mode::Preview | Mode::Help => tab.line_index = tab.window.top,
            _ => (),
        }
        tab.window.scroll_up_one(tab.line_index);
    }

    pub fn event_down_one_row(tab: &mut Tab) {
        match tab.mode {
            Mode::Normal => {
                tab.path_content.select_next();
                let max_line = tab.path_content.files.len();
                if max_line >= ContentWindow::WINDOW_MARGIN_TOP
                    && tab.line_index < max_line - ContentWindow::WINDOW_MARGIN_TOP
                {
                    tab.line_index += 1;
                }
            }
            Mode::Preview | Mode::Help => tab.line_index = tab.window.bottom,
            _ => (),
        }
        tab.window.scroll_down_one(tab.line_index);
    }

    pub fn event_go_top(tab: &mut Tab) {
        if let Mode::Normal = tab.mode {
            tab.path_content.select_index(0);
        }
        tab.line_index = 0;
        tab.window.scroll_to(0);
    }

    pub fn event_page_up(tab: &mut Tab) {
        let scroll_up: usize = if let Mode::Normal = tab.mode {
            10
        } else {
            tab.height
        };
        let up_index = if tab.line_index > scroll_up {
            tab.line_index - scroll_up
        } else {
            0
        };
        if let Mode::Normal = tab.mode {
            tab.path_content.select_index(up_index);
        }
        tab.line_index = up_index;
        tab.window.scroll_to(up_index);
    }

    pub fn event_go_bottom(tab: &mut Tab) {
        let last_index: usize;
        if let Mode::Normal = tab.mode {
            last_index = tab.path_content.files.len() - 1;
            tab.path_content.select_index(last_index);
        } else {
            last_index = tab.preview.len() - 1;
        }
        tab.line_index = last_index;
        tab.window.scroll_to(last_index);
    }

    pub fn event_cursor_home(tab: &mut Tab) {
        tab.input.cursor_start()
    }

    pub fn event_cursor_end(tab: &mut Tab) {
        tab.input.cursor_end()
    }

    pub fn event_page_down(tab: &mut Tab) {
        let down_index: usize;
        if let Mode::Normal = tab.mode {
            down_index = min(tab.path_content.files.len() - 1, tab.line_index + 10);
            tab.path_content.select_index(down_index);
        } else {
            down_index = min(tab.preview.len() - 1, tab.line_index + 30)
        }
        tab.line_index = down_index;
        tab.window.scroll_to(down_index);
    }

    pub fn event_select_row(tab: &mut Tab, row: u16) {
        tab.line_index = (row - 2).into();
        tab.path_content.select_index(tab.line_index);
        tab.window.scroll_to(tab.line_index)
    }

    pub fn event_shortcut_next(tab: &mut Tab) {
        tab.shortcut.next()
    }

    pub fn event_shortcut_prev(tab: &mut Tab) {
        tab.shortcut.prev()
    }

    pub fn event_history_next(tab: &mut Tab) {
        tab.history.next()
    }

    pub fn event_history_prev(tab: &mut Tab) {
        tab.history.prev()
    }

    pub fn event_move_to_parent(tab: &mut Tab) -> FmResult<()> {
        let parent = tab.path_content.path.parent();
        let path = path::PathBuf::from(
            parent.ok_or_else(|| FmError::new("Root directory has no parent"))?,
        );
        tab.history.push(&path);
        tab.path_content = PathContent::new(path, tab.show_hidden)?;
        tab.window.reset(tab.path_content.files.len());
        tab.line_index = 0;
        tab.input.cursor_start();
        Ok(())
    }

    pub fn event_move_cursor_left(tab: &mut Tab) {
        tab.input.cursor_left()
    }

    pub fn exec_file(tab: &mut Tab) -> FmResult<()> {
        if tab.path_content.is_empty() {
            return Ok(());
        }
        if tab.path_content.is_selected_dir()? {
            tab.go_to_child()
        } else {
            Self::event_open_file(tab)
        }
    }

    pub fn event_move_cursor_right(tab: &mut Tab) {
        tab.input.cursor_right()
    }

    pub fn event_delete_char_left(tab: &mut Tab) {
        tab.input.delete_char_left()
    }

    pub fn event_delete_chars_right(tab: &mut Tab) {
        tab.input.delete_chars_right()
    }

    pub fn event_text_insert_and_complete(tab: &mut Tab, c: char) -> FmResult<()> {
        Self::event_text_insertion(tab, c);
        tab.fill_completion()
    }

    pub fn event_copy_paste(tab: &mut Tab) {
        tab.mode = Mode::NeedConfirmation;
        tab.last_edition = LastEdition::CopyPaste;
    }

    pub fn event_cur_paste(tab: &mut Tab) {
        tab.mode = Mode::NeedConfirmation;
        tab.last_edition = LastEdition::CutPaste;
    }

    pub fn event_new_dir(tab: &mut Tab) {
        tab.mode = Mode::Newdir
    }

    pub fn event_new_file(tab: &mut Tab) {
        tab.mode = Mode::Newfile
    }

    pub fn event_exec(tab: &mut Tab) {
        tab.mode = Mode::Exec
    }

    pub fn event_preview(tab: &mut Tab) -> FmResult<()> {
        if tab.path_content.files.is_empty() {
            return Err(FmError::new("No file to preview"));
        }
        if let Some(file) = tab.path_content.selected_file() {
            if let FileKind::NormalFile = file.file_kind {
                tab.mode = Mode::Preview;
                tab.preview = Preview::new(&tab.path_content)?;
                tab.window.reset(tab.preview.len());
            }
        }
        Ok(())
    }

    pub fn event_delete_file(tab: &mut Tab) {
        tab.mode = Mode::NeedConfirmation;
        tab.last_edition = LastEdition::Delete;
    }

    pub fn event_help(tab: &mut Tab) {
        tab.mode = Mode::Help;
        tab.preview = Preview::help(tab.help.clone());
        tab.window.reset(tab.preview.len())
    }

    pub fn event_search(tab: &mut Tab) {
        tab.mode = Mode::Search
    }

    pub fn event_regex_match(tab: &mut Tab) {
        tab.mode = Mode::RegexMatch
    }

    pub fn event_sort(tab: &mut Tab) {
        tab.mode = Mode::Sort;
    }

    pub fn event_quit(tab: &mut Tab) {
        tab.must_quit = true
    }

    pub fn event_leave_need_confirmation(tab: &mut Tab) {
        tab.last_edition = LastEdition::Nothing;
        tab.mode = Mode::Normal;
    }

    pub fn event_leave_sort(tab: &mut Tab, c: char) {
        tab.mode = Mode::Normal;
        match c {
            'k' => tab.path_content.sort_by = SortBy::Kind,
            'n' => tab.path_content.sort_by = SortBy::Filename,
            'm' => tab.path_content.sort_by = SortBy::Date,
            's' => tab.path_content.sort_by = SortBy::Size,
            'e' => tab.path_content.sort_by = SortBy::Extension,
            'r' => tab.path_content.reverse = !tab.path_content.reverse,
            _ => {
                return;
            }
        }
        if !tab.path_content.files.is_empty() {
            tab.path_content.files[tab.line_index].unselect();
            tab.path_content.sort();
            if tab.path_content.reverse {
                tab.path_content.files.reverse();
            }
            Self::event_go_top(tab);
            tab.path_content.select_index(0)
        }
    }

    pub fn event_text_insertion(tab: &mut Tab, c: char) {
        tab.input.insert(c);
    }

    pub fn event_toggle_hidden(tab: &mut Tab) -> FmResult<()> {
        tab.show_hidden = !tab.show_hidden;
        tab.path_content.show_hidden = !tab.path_content.show_hidden;
        tab.path_content.reset_files()?;
        tab.line_index = 0;
        tab.window.reset(tab.path_content.files.len());
        Ok(())
    }

    pub fn event_open_file(tab: &mut Tab) -> FmResult<()> {
        match tab.opener.open(
            tab.path_content
                .selected_file()
                .ok_or_else(|| FmError::new("Empty directory"))?
                .path
                .clone(),
        ) {
            Ok(_) => (),
            Err(e) => info!(
                "Error opening {:?}: {:?}",
                tab.path_content.selected_file(),
                e
            ),
        }
        Ok(())
    }

    pub fn event_rename(tab: &mut Tab) {
        tab.mode = Mode::Rename;
    }

    pub fn event_goto(tab: &mut Tab) {
        tab.mode = Mode::Goto;
        tab.completion.reset();
    }

    pub fn event_shell(tab: &mut Tab) -> FmResult<()> {
        execute_in_child(
            &tab.terminal,
            &vec![
                "-d",
                tab.path_content
                    .path
                    .to_str()
                    .ok_or_else(|| FmError::new("Couldn't parse the path name"))?,
            ],
        )?;
        Ok(())
    }

    pub fn event_history(tab: &mut Tab) {
        tab.mode = Mode::History
    }

    pub fn event_shortcut(tab: &mut Tab) {
        tab.mode = Mode::Shortcut
    }

    pub fn event_right_click(tab: &mut Tab, row: u16) -> FmResult<()> {
        if tab.path_content.files.is_empty() || row as usize > tab.path_content.files.len() + 1 {
            return Err(FmError::new("not found"));
        }
        tab.line_index = (row - 2).into();
        tab.path_content.select_index(tab.line_index);
        tab.window.scroll_to(tab.line_index);
        if let FileKind::Directory = tab
            .path_content
            .selected_file()
            .ok_or_else(|| FmError::new("not found"))?
            .file_kind
        {
            Self::exec_file(tab)
        } else {
            Self::event_open_file(tab)
        }
    }

    pub fn event_replace_input_with_completion(tab: &mut Tab) {
        tab.input.replace(tab.completion.current_proposition())
    }

    pub fn event_nvim_filepicker(tab: &mut Tab) {
        if tab.path_content.files.is_empty() {
            info!("Called nvim filepicker in an empty directory.");
            return;
        }
        // "nvim-send --remote-send '<esc>:e readme.md<cr>' --servername 127.0.0.1:8888"
        if let Ok(nvim_listen_address) = Self::nvim_listen_address(tab) {
            if let Some(path_str) = tab.path_content.selected_path_str() {
                let _ = execute_in_child(
                    "nvim-send",
                    &vec![
                        "--remote-send",
                        &format!("<esc>:e {}<cr><esc>:close<cr>", path_str),
                        "--servername",
                        &nvim_listen_address,
                    ],
                );
            }
        } else {
            info!("Nvim server not defined");
        }
    }

    pub fn event_filename_to_clipboard(tab: &Tab) -> FmResult<()> {
        if let Some(file) = tab.path_content.selected_file() {
            let filename = file.filename.clone();
            let mut ctx = ClipboardContext::new()?;
            ctx.set_contents(filename)?;
            // For some reason, it's not writen if you don't read it back...
            let _ = ctx.get_contents();
        }
        Ok(())
    }

    pub fn event_filepath_to_clipboard(tab: &Tab) -> FmResult<()> {
        if let Some(filepath) = tab.path_content.selected_path_str() {
            let mut ctx = ClipboardContext::new()?;
            ctx.set_contents(filepath)?;
            // For some reason, it's not writen if you don't read it back...
            let _ = ctx.get_contents();
        }
        Ok(())
    }

    pub fn event_filter(tab: &mut Tab) -> FmResult<()> {
        tab.mode = Mode::Filter;
        Ok(())
    }

    pub fn event_decompress(tab: &mut Tab) -> FmResult<()> {
        if let Some(fileinfo) = tab.path_content.selected_file() {
            decompress(&fileinfo.path)
        } else {
            Ok(())
        }
    }

    pub fn event_back(tab: &mut Tab) -> FmResult<()> {
        if tab.history.visited.len() <= 1 {
            return Ok(());
        }
        tab.history.visited.pop();
        let last = tab.history.visited[tab.history.len() - 1].clone();
        tab.set_pathcontent(last)?;

        Ok(())
    }

    pub fn event_home(tab: &mut Tab) -> FmResult<()> {
        let home_cow = shellexpand::tilde("~");
        let home: &str = home_cow.borrow();
        let path = std::fs::canonicalize(home)?;
        tab.set_pathcontent(path)?;

        Ok(())
    }

    fn nvim_listen_address(tab: &Tab) -> Result<String, std::env::VarError> {
        if !tab.nvim_server.is_empty() {
            Ok(tab.nvim_server.clone())
        } else {
            std::env::var("NVIM_LISTEN_ADDRESS")
        }
    }

    pub fn exec_rename(tab: &mut Tab) -> FmResult<()> {
        if tab.path_content.files.is_empty() {
            return Err(FmError::new("Empty directory"));
        }
        fs::rename(
            tab.path_content
                .selected_path_str()
                .ok_or_else(|| FmError::new("File not found"))?,
            tab.path_content.path.to_path_buf().join(&tab.input.string),
        )?;
        tab.refresh_view()
    }

    pub fn exec_newfile(tab: &mut Tab) -> FmResult<()> {
        fs::File::create(tab.path_content.path.join(tab.input.string.clone()))?;
        tab.refresh_view()
    }

    pub fn exec_newdir(tab: &mut Tab) -> FmResult<()> {
        match fs::create_dir(tab.path_content.path.join(tab.input.string.clone())) {
            Ok(()) => (),
            Err(e) => match e.kind() {
                std::io::ErrorKind::AlreadyExists => (),
                _ => return Err(FmError::from(e)),
            },
        }
        tab.refresh_view()
    }

    pub fn exec_exec(tab: &mut Tab) -> FmResult<()> {
        if tab.path_content.files.is_empty() {
            return Err(FmError::new("empty directory"));
        }
        let exec_command = tab.input.string.clone();
        let mut args: Vec<&str> = exec_command.split(' ').collect();
        let command = args.remove(0);
        if std::path::Path::new(command).exists() {
            let path = &tab
                .path_content
                .selected_path_str()
                .ok_or_else(|| FmError::new("path unreachable"))?;
            args.push(path);
            execute_in_child(command, &args)?;
            tab.completion.reset();
            tab.input.reset();
        }
        Ok(())
    }

    pub fn event_drag_n_drop(tab: &mut Tab) -> FmResult<()> {
        execute_in_child(
            "dragon-drop",
            &vec![&tab
                .path_content
                .selected_path_str()
                .ok_or_else(|| FmError::new("path unreachable"))?],
        )?;
        Ok(())
    }

    pub fn exec_search(tab: &mut Tab) {
        tab.input.reset();
        let completed = tab.completion.current_proposition();
        if completed.is_empty() {
            return;
        }
        let mut next_index = tab.line_index;
        for (index, file) in tab.path_content.files.iter().enumerate().skip(next_index) {
            if file.filename == completed {
                next_index = index;
                break;
            };
        }
        tab.path_content.select_index(next_index);
        tab.line_index = next_index;
        tab.window.scroll_to(tab.line_index);
    }

    pub fn exec_goto(tab: &mut Tab) -> FmResult<()> {
        let target_string = tab.input.string.clone();
        tab.input.reset();
        let expanded_cow_path = shellexpand::tilde(&target_string);
        let expanded_target: &str = expanded_cow_path.borrow();
        let path = std::fs::canonicalize(expanded_target)?;
        tab.history.push(&path);
        tab.path_content = PathContent::new(path, tab.show_hidden)?;
        tab.window.reset(tab.path_content.files.len());
        Ok(())
    }

    pub fn exec_shortcut(tab: &mut Tab) -> FmResult<()> {
        tab.input.reset();
        let path = tab.shortcut.selected();
        tab.history.push(&path);
        tab.path_content = PathContent::new(path, tab.show_hidden)?;
        Self::event_normal(tab)
    }

    pub fn exec_history(tab: &mut Tab) -> FmResult<()> {
        tab.input.reset();
        tab.path_content = PathContent::new(
            tab.history
                .selected()
                .ok_or_else(|| FmError::new("path unreachable"))?,
            tab.show_hidden,
        )?;
        tab.history.drop_queue();
        Self::event_normal(tab)
    }

    pub fn exec_filter(tab: &mut Tab) -> FmResult<()> {
        let filter = FilterKind::from_input(&tab.input.string);
        tab.path_content.set_filter(filter);
        tab.input.reset();
        tab.path_content.reset_files()?;
        Self::event_normal(tab)
    }
}
