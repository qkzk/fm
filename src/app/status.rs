use std::cmp::min;
use std::fs;
use std::path::Path;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use skim::SkimItem;
use sysinfo::{Disk, RefreshKind, System, SystemExt};
use tuikit::prelude::{from_keyname, Event};
use tuikit::term::Term;

use crate::app::FlaggedHeader;
use crate::app::Footer;
use crate::app::Header;
use crate::app::InternalSettings;
use crate::app::Session;
use crate::app::Tab;
use crate::app::{ClickableLine, FlaggedFooter};
use crate::common::{
    args_is_empty, filename_from_path, is_sudo_command, open_in_current_neovim, path_to_string,
};
use crate::common::{current_username, disk_space, is_program_in_path};
use crate::config::Bindings;
use crate::event::FmEvents;
use crate::io::Internal;
use crate::io::Opener;
use crate::io::MIN_WIDTH_FOR_DUAL_PANE;
use crate::io::{execute_and_capture_output_with_path, Args};
use crate::io::{
    execute_and_capture_output_without_check, execute_sudo_command_with_password,
    reset_sudo_faillock,
};
use crate::io::{Extension, Kind};
use crate::modes::IsoDevice;
use crate::modes::MountCommands;
use crate::modes::MountRepr;
use crate::modes::NeedConfirmation;
use crate::modes::PasswordKind;
use crate::modes::PasswordUsage;
use crate::modes::Permissions;
use crate::modes::Preview;
use crate::modes::Selectable;
use crate::modes::ShellCommandParser;
use crate::modes::Skimer;
use crate::modes::To;
use crate::modes::Tree;
use crate::modes::Users;
use crate::modes::{copy_move, regex_matcher};
use crate::modes::{extract_extension, Edit};
use crate::modes::{BlockDeviceAction, Navigate};
use crate::modes::{Content, FileInfo};
use crate::modes::{ContentWindow, CopyMove};
use crate::modes::{Display, Go};
use crate::modes::{FilterKind, InputSimple};
use crate::modes::{Menu, PreviewHolder};
use crate::{log_info, log_line};

pub enum Window {
    Header,
    Files,
    Menu,
    Footer,
}

#[derive(Default, Clone, Copy)]
pub enum Focus {
    #[default]
    LeftFile,
    LeftMenu,
    RightFile,
    RightMenu,
}

impl Focus {
    pub fn is_left(&self) -> bool {
        matches!(self, Self::LeftMenu | Self::LeftFile)
    }

    pub fn is_file(&self) -> bool {
        matches!(self, Self::LeftFile | Self::RightFile)
    }

    pub fn switch(&self) -> Self {
        match self {
            Self::LeftFile => Self::RightFile,
            Self::LeftMenu => Self::RightFile,
            Self::RightFile => Self::LeftFile,
            Self::RightMenu => Self::LeftFile,
        }
    }

    pub fn to_parent(&self) -> Self {
        match self {
            Self::LeftFile | Self::LeftMenu => Self::LeftFile,
            Self::RightFile | Self::RightMenu => Self::RightFile,
        }
    }

    pub fn index(&self) -> usize {
        *self as usize
    }
}

/// Holds every mutable parameter of the application itself, except for
/// the "display" information.
/// It holds 2 tabs (left & right), even if only one can be displayed sometimes.
/// It knows which tab is selected, which files are flagged,
/// which jump target is selected, a cache of normal file colors,
/// if we have to display one or two tabs and if all details are shown or only
/// the filename.
/// Mutation of this struct are mostly done externally, by the event crate :
/// `crate::event_exec`.
pub struct Status {
    /// Vector of `Tab`, each of them are displayed in a separate tab.
    pub tabs: [Tab; 2],
    /// Index of the current selected tab
    pub index: usize,

    skimer: Option<Result<Skimer>>,

    /// Navigable menu
    pub menu: Menu,
    /// Display settings
    pub display_settings: Session,
    /// Internal settings
    pub internal_settings: InternalSettings,
    /// Window being focused currently
    pub focus: Focus,
    /// Sender of events
    pub fm_sender: Arc<Sender<FmEvents>>,
    pub preview_holder: Arc<Mutex<PreviewHolder>>,
}

impl Status {
    /// Creates a new status for the application.
    /// It requires most of the information (arguments, configuration, height
    /// of the terminal, the formated help string).
    pub fn new(
        height: usize,
        term: Arc<Term>,
        opener: Opener,
        binds: &Bindings,
        fm_sender: Arc<Sender<FmEvents>>,
        preview_holder: Arc<Mutex<PreviewHolder>>,
    ) -> Result<Self> {
        let skimer = None;
        let index = 0;

        let args = Args::parse();
        let path = std::fs::canonicalize(Path::new(&args.path))?;
        let start_dir = if path.is_dir() {
            &path
        } else {
            path.parent().context("")?
        };
        let sys = System::new_with_specifics(RefreshKind::new().with_disks());
        let display_settings = Session::new(term.term_size()?.0);
        let mut internal_settings = InternalSettings::new(opener, term, sys);
        let mount_points = internal_settings.mount_points();
        let menu = Menu::new(start_dir, &mount_points, binds, fm_sender.clone())?;

        let users_left = Users::new();
        let users_right = users_left.clone();

        let tabs = [
            Tab::new(&args, height, users_left)?,
            Tab::new(&args, height, users_right)?,
        ];
        let focus = Focus::default();
        Ok(Self {
            tabs,
            index,
            skimer,
            menu,
            display_settings,
            internal_settings,
            focus,
            fm_sender,
            preview_holder,
        })
    }

    /// Returns a non mutable reference to the selected tab.
    pub fn current_tab(&self) -> &Tab {
        &self.tabs[self.index]
    }

    /// Returns a mutable reference to the selected tab.
    pub fn current_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.index]
    }

    /// Returns a string representing the current path in the selected tab.
    pub fn current_tab_path_str(&self) -> String {
        self.current_tab().directory_str()
    }

    /// True if a quit event was registered in the selected tab.
    pub fn must_quit(&self) -> bool {
        self.internal_settings.must_quit
    }

    pub fn switch_focus(&mut self) {
        if (self.index == 0 && !self.focus.is_left()) || (self.index == 1 && self.focus.is_left()) {
            self.focus = self.focus.switch();
        }
    }

    pub fn set_focus_from_mode(&mut self) {
        if self.index == 0 {
            if matches!(self.tabs[self.index].edit_mode, Edit::Nothing) {
                self.focus = Focus::LeftFile;
            } else {
                self.focus = Focus::LeftMenu;
            }
        } else if matches!(self.tabs[self.index].edit_mode, Edit::Nothing) {
            self.focus = Focus::RightFile;
        } else {
            self.focus = Focus::RightMenu;
        }
    }

    /// Select the other tab if two are displayed. Does nother otherwise.
    pub fn next(&mut self) {
        if !self.display_settings.dual() {
            return;
        }
        self.index = 1 - self.index;
        self.switch_focus();
    }

    /// Select the other tab if two are displayed. Does nother otherwise.
    pub fn prev(&mut self) {
        self.next();
    }

    /// Select the left or right tab depending on where the user clicked.
    pub fn select_tab_from_col(&mut self, col: u16) -> Result<()> {
        let (width, _) = self.term_size()?;
        if self.display_settings.dual() {
            if (col as usize) < width / 2 {
                self.select_left();
            } else {
                self.select_right();
            };
        } else {
            self.select_left();
        }
        Ok(())
    }

    fn window_from_row(&self, row: u16, height: usize) -> Window {
        let win_height = if matches!(self.current_tab().edit_mode, Edit::Nothing) {
            height
        } else {
            height / 2
        };
        let w_index = row as usize / win_height;
        if w_index == 1 {
            Window::Menu
        } else if row == 1 {
            Window::Header
        } else if row as usize == win_height - 2 {
            Window::Footer
        } else {
            Window::Files
        }
    }

    pub fn set_focus(&mut self, row: u16, col: u16) -> Result<Window> {
        self.select_tab_from_col(col)?;
        let window = self.window_from_row(row, self.term_size()?.1);
        self.set_focus_from_window_and_index(&window);
        Ok(window)
    }

    /// Execute a click at `row`, `col`. Action depends on which window was clicked.
    pub fn click(&mut self, row: u16, col: u16, binds: &Bindings) -> Result<()> {
        let window = self.set_focus(row, col)?;
        self.click_action_from_window(&window, row, col, binds)?;
        Ok(())
    }

    fn click_action_from_window(
        &mut self,
        window: &Window,
        row: u16,
        col: u16,
        binds: &Bindings,
    ) -> Result<()> {
        match window {
            Window::Header => self.header_action(col, binds),
            Window::Files => {
                if matches!(self.current_tab().display_mode, Display::Flagged) {
                    self.menu.flagged.select_row(row)
                } else {
                    self.current_tab_mut().select_row(row)?
                }
                self.update_second_pane_for_preview()
            }
            Window::Footer => self.footer_action(col, binds),
            Window::Menu => self.menu_action(row),
        }
    }

    fn set_focus_from_window_and_index(&mut self, window: &Window) {
        self.focus = if self.index == 0 {
            if matches!(window, Window::Menu) {
                Focus::LeftMenu
            } else {
                Focus::LeftFile
            }
        } else if matches!(window, Window::Menu) {
            Focus::RightMenu
        } else {
            Focus::RightFile
        };
    }

    pub fn second_window_height(&self) -> Result<usize> {
        let (_, height) = self.term_size()?;
        Ok(height / 2 + (height % 2))
    }

    /// Execute a click on a menu item. Action depends on which menu was opened.
    fn menu_action(&mut self, row: u16) -> Result<()> {
        let second_window_height = self.second_window_height()?;
        let offset = row as usize - second_window_height;
        if offset >= 4 {
            let index = offset - 4 + self.menu.window.top;
            match self.current_tab().edit_mode {
                Edit::Navigate(navigate) => match navigate {
                    Navigate::CliApplication => self.menu.cli_applications.set_index(index),
                    Navigate::Compress => self.menu.compression.set_index(index),
                    Navigate::Context => self.menu.context.set_index(index),
                    Navigate::EncryptedDrive => self.menu.encrypted_devices.set_index(index),
                    Navigate::History => self.current_tab_mut().history.set_index(index),
                    Navigate::Marks(_) => self.menu.marks.set_index(index),
                    Navigate::RemovableDevices => self.menu.removable_devices.set_index(index),
                    Navigate::Shortcut => self.menu.shortcut.set_index(index),
                    Navigate::Trash => self.menu.trash.set_index(index),
                    Navigate::TuiApplication => self.menu.tui_applications.set_index(index),
                },
                Edit::InputCompleted(_) => self.menu.completion.set_index(index),
                _ => (),
            }
            self.menu.window.scroll_to(index);
        }
        Ok(())
    }

    /// Select the left tab
    pub fn select_left(&mut self) {
        self.index = 0;
        self.switch_focus();
    }

    /// Select the right tab
    pub fn select_right(&mut self) {
        self.index = 1;
        self.switch_focus();
    }

    /// Refresh every disk information.
    /// It also refreshes the disk list, which is usefull to detect removable medias.
    /// It may be very slow...
    /// There's surelly a better way, like doing it only once in a while or on
    /// demand.
    pub fn refresh_shortcuts(&mut self) {
        self.menu.refresh_shortcuts(
            &self.internal_settings.mount_points(),
            self.tabs[0].current_path(),
            self.tabs[1].current_path(),
        );
    }

    /// Returns an array of Disks
    pub fn disks(&self) -> &[Disk] {
        self.internal_settings.sys.disks()
    }

    /// Returns a the disk spaces for the selected tab..
    pub fn disk_spaces_of_selected(&self) -> String {
        disk_space(self.disks(), self.current_tab().current_path())
    }

    /// Returns the sice of the terminal (width, height)
    pub fn term_size(&self) -> Result<(usize, usize)> {
        self.internal_settings.term_size()
    }

    /// Refresh the current view, reloading the files. Move the selection to top.
    pub fn refresh_view(&mut self) -> Result<()> {
        self.refresh_status()?;
        self.update_second_pane_for_preview()
    }

    /// Reset the view of every tab.
    pub fn reset_tabs_view(&mut self) -> Result<()> {
        for tab in self.tabs.iter_mut() {
            tab.refresh_and_reselect_file()?
        }
        Ok(())
    }

    /// Reset the edit mode to "Nothing" (closing any menu) and returns
    /// true if the display should be refreshed.
    pub fn reset_edit_mode(&mut self) -> Result<bool> {
        self.menu.reset();
        let must_refresh = matches!(self.current_tab().display_mode, Display::Preview);
        self.set_edit_mode(self.index, Edit::Nothing)?;
        Ok(must_refresh)
    }

    /// Reset the selected tab view to the default.
    pub fn refresh_status(&mut self) -> Result<()> {
        self.force_clear();
        self.refresh_users()?;
        self.refresh_tabs()?;
        Ok(())
    }

    /// Set a "force clear" flag to true, which will reset the display.
    /// It's used when some command or whatever may pollute the terminal.
    /// We ensure to clear it before displaying again.
    pub fn force_clear(&mut self) {
        self.internal_settings.force_clear();
    }

    /// Refresh the users for every tab
    pub fn refresh_users(&mut self) -> Result<()> {
        let users = Users::new();
        self.tabs[0].users = users.clone();
        self.tabs[1].users = users;
        Ok(())
    }

    /// Refresh the input, completion and every tab.
    pub fn refresh_tabs(&mut self) -> Result<()> {
        self.menu.input.reset();
        self.menu.completion.reset();
        self.tabs[0].refresh_and_reselect_file()?;
        self.tabs[1].refresh_and_reselect_file()
    }

    /// When a rezise event occurs, we may hide the second panel if the width
    /// isn't sufficiant to display enough information.
    /// We also need to know the new height of the terminal to start scrolling
    /// up or down.
    pub fn resize(&mut self, width: usize, height: usize) -> Result<()> {
        self.set_dual_pane_if_wide_enough(width)?;
        self.tabs[0].set_height(height);
        self.tabs[1].set_height(height);
        self.refresh_status()
    }

    /// Drop the current tree, replace it with an empty one.
    pub fn remove_tree(&mut self) -> Result<()> {
        self.current_tab_mut().tree = Tree::default();
        Ok(())
    }

    /// Check if the second pane should display a preview and force it.
    pub fn update_second_pane_for_preview(&mut self) -> Result<()> {
        if self.index == 0 && self.display_settings.preview() {
            if Session::display_wide_enough(self.term_size()?.0) {
                self.set_second_pane_for_preview()?;
            } else {
                // self.tabs[1].preview = Preview::empty();
                self.tabs[1].reset_preview();
            }
        };
        Ok(())
    }

    /// Force preview the selected file of the first pane in the second pane.
    /// Doesn't check if it has do.
    fn set_second_pane_for_preview(&mut self) -> Result<()> {
        self.tabs[1].set_display_mode(Display::Preview);
        self.tabs[1].edit_mode = Edit::Nothing;
        let Ok(fileinfo) = self.get_correct_fileinfo_for_preview() else {
            return Ok(());
        };
        let Ok(mut preview_holder) = self.preview_holder.lock() else {
            return Ok(());
        };
        preview_holder.start_previewer();
        preview_holder.build(&fileinfo)?;
        self.tabs[1].previewed_doc = Some(path_to_string(&fileinfo.path));
        // self.tabs[1].preview = Preview::new(&fileinfo, users).unwrap_or_default();
        // self.tabs[1].window.reset(self.tabs[1].preview.len());
        let len = match preview_holder.get(&fileinfo.path) {
            Some(preview) => preview.len(),
            _ => 80,
        };
        self.tabs[1].window.reset(len);
        Ok(())
    }

    pub fn start_previewer(&mut self) {
        log_info!("status locking preview_holder...");
        let Ok(mut preview_holder) = self.preview_holder.lock() else {
            log_info!("status couldn't lock preview_holder");
            return;
        };
        preview_holder.start_previewer();
    }

    pub fn build_preview_current_tab(&mut self) -> Result<()> {
        log_info!("status locking preview_holder...");
        let Ok(mut preview_holder) = self.preview_holder.lock() else {
            log_info!("status couldn't lock preview_holder");
            return Ok(());
        };
        let Ok(fileinfo) = self.current_tab().current_file() else {
            return Ok(());
        };
        let len = match preview_holder.get(&fileinfo.path) {
            Some(preview) => preview.len(),
            _ => 80,
        };
        self.tabs[self.index].previewed_doc = Some(path_to_string(&fileinfo.path));
        log_info!("build_preview_current_tab. Preview has len {len}");
        self.tabs[self.index].window.reset(len);
        preview_holder.build(&fileinfo)
    }

    fn get_correct_fileinfo_for_preview(&mut self) -> Result<FileInfo> {
        let left_tab = &self.tabs[0];
        let users = &left_tab.users;
        match self.focus {
            Focus::LeftMenu if matches!(left_tab.edit_mode, Edit::Navigate(Navigate::Marks(_))) => {
                let (_, mark_path) = &self.menu.marks.content()[self.menu.marks.index()];
                FileInfo::new(mark_path, users)
            }
            Focus::LeftMenu if matches!(left_tab.edit_mode, Edit::Navigate(Navigate::Shortcut)) => {
                let shortcut_path = &self.menu.shortcut.content()[self.menu.shortcut.index()];
                FileInfo::new(shortcut_path, users)
            }
            Focus::LeftMenu if matches!(left_tab.edit_mode, Edit::Navigate(Navigate::History)) => {
                let (history_path, _) = &left_tab.history.content()[left_tab.history.index()];
                FileInfo::new(history_path, users)
            }
            _ => match left_tab.display_mode {
                Display::Flagged => {
                    let Some(path) = self.menu.flagged.selected() else {
                        // self.tabs[1].preview = Preview::empty();
                        return Err(anyhow!("No fileinfo to preview"));
                    };
                    FileInfo::new(path, users)
                }
                _ => left_tab.current_file(),
            },
        }
    }

    /// Set an edit mode for the tab at `index`. Refresh the view.
    pub fn set_edit_mode(&mut self, index: usize, edit_mode: Edit) -> Result<()> {
        if index > 1 {
            return Ok(());
        }
        self.set_height_for_edit_mode(index, edit_mode)?;
        self.tabs[index].edit_mode = edit_mode;
        let len = self.menu.len(edit_mode);
        let height = self.second_window_height()?;
        self.menu.window = ContentWindow::new(len, height);
        self.set_focus_from_mode();
        self.menu.input_history.filter_by_mode(edit_mode);
        self.refresh_status()
    }

    pub fn set_height_for_edit_mode(&mut self, index: usize, edit_mode: Edit) -> Result<()> {
        let height = self.internal_settings.term.term_size()?.1;
        let prim_window_height = if matches!(edit_mode, Edit::Nothing) {
            height
        } else {
            height / 2
        };
        self.tabs[index].window.set_height(prim_window_height);
        self.tabs[index]
            .window
            .scroll_to(self.tabs[index].window.top);
        Ok(())
    }

    /// Set dual pane if the term is big enough
    pub fn set_dual_pane_if_wide_enough(&mut self, width: usize) -> Result<()> {
        if width < MIN_WIDTH_FOR_DUAL_PANE {
            self.select_left();
            self.display_settings.set_dual(false);
        } else {
            self.display_settings.set_dual(true);
        }
        Ok(())
    }

    /// Empty the flagged files, reset the view of every tab.
    pub fn clear_flags_and_reset_view(&mut self) -> Result<()> {
        self.menu.flagged.clear();
        self.reset_tabs_view()
    }

    /// Returns a vector of path of files which are both flagged and in current
    /// directory.
    /// It's necessary since the user may have flagged files OUTSIDE of current
    /// directory before calling Bulkrename.
    /// It may be confusing since the same filename can be used in
    /// different places.
    pub fn flagged_in_current_dir(&self) -> Vec<std::path::PathBuf> {
        self.menu.flagged.in_dir(&self.current_tab().directory.path)
    }

    /// Flag all files in the current directory or current tree.
    pub fn flag_all(&mut self) {
        match self.current_tab().display_mode {
            Display::Directory => {
                self.tabs[self.index]
                    .directory
                    .content
                    .iter()
                    .for_each(|file| {
                        self.menu.flagged.push(file.path.to_path_buf());
                    });
            }
            Display::Tree => self.tabs[self.index].tree.flag_all(&mut self.menu.flagged),
            _ => (),
        }
    }

    /// Reverse every flag in _current_ directory. Flagged files in other
    /// directory aren't affected.
    pub fn reverse_flags(&mut self) {
        match self.current_tab().display_mode {
            Display::Preview => (),
            Display::Tree => (),
            Display::Flagged => (),
            Display::Directory => {
                self.tabs[self.index]
                    .directory
                    .content
                    .iter()
                    .for_each(|file| self.menu.flagged.toggle(&file.path));
            }
        }
    }

    /// Flag the selected file if any
    pub fn toggle_flag_for_selected(&mut self) {
        let tab = self.current_tab();

        if matches!(tab.edit_mode, Edit::Nothing) {
            let Ok(file) = tab.current_file() else {
                return;
            };
            match tab.display_mode {
                Display::Flagged => {
                    self.menu.flagged.remove_selected();
                }
                Display::Directory => {
                    self.menu.flagged.toggle(&file.path);
                    if !self.tabs[self.index].directory.selected_is_last() {
                        self.current_tab_mut().normal_down_one_row();
                    }
                    let _ = self.update_second_pane_for_preview();
                }
                Display::Tree => {
                    self.menu.flagged.toggle(&file.path);
                    if !self.tabs[self.index].tree.selected_is_last() {
                        let _ = self.current_tab_mut().tree_select_next();
                    }
                    let _ = self.update_second_pane_for_preview();
                }
                Display::Preview => (),
            }
        }
    }

    /// Execute a move or a copy of the flagged files to current directory.
    /// A progress bar is displayed (invisible for small files) and a notification
    /// is sent every time, even for 0 bytes files...
    pub fn cut_or_copy_flagged_files(&mut self, cut_or_copy: CopyMove) -> Result<()> {
        let sources = self.menu.flagged.content.clone();
        let dest = &self.current_tab().directory_of_selected()?.to_owned();

        if self.is_simple_move(&cut_or_copy, &sources, dest) {
            return self.simple_move(&sources, dest);
        }

        let mut must_act_now = true;
        if matches!(cut_or_copy, CopyMove::Copy) {
            if !self.internal_settings.copy_file_queue.is_empty() {
                log_info!("cut_or_copy_flagged_files: act later");
                must_act_now = false;
            }
            self.internal_settings
                .copy_file_queue
                .push((sources.to_owned(), dest.clone()));
        }

        if must_act_now {
            log_info!("cut_or_copy_flagged_files: act now");
            let in_mem = copy_move(
                cut_or_copy,
                sources,
                dest,
                Arc::clone(&self.internal_settings.term),
                Arc::clone(&self.fm_sender),
            )?;
            self.internal_settings.store_copy_progress(in_mem);
        }
        self.clear_flags_and_reset_view()
    }

    fn is_simple_move(
        &self,
        cut_or_copy: &CopyMove,
        sources: &[std::path::PathBuf],
        dest: &std::path::Path,
    ) -> bool {
        matches!(cut_or_copy, CopyMove::Move)
            && sources.len() == 1
            && crate::common::disk_used_by_path(self.disks(), &sources[0])
                == crate::common::disk_used_by_path(self.disks(), dest)
    }

    fn simple_move(
        &mut self,
        sources: &[std::path::PathBuf],
        dest: &std::path::Path,
    ) -> Result<()> {
        let source = &sources[0];
        let filename = filename_from_path(source)?;
        let dest = dest.to_path_buf().join(filename);
        match std::fs::rename(source, &dest) {
            Ok(()) => {
                log_line!(
                    "Moved {source} to {dest}",
                    source = source.display(),
                    dest = dest.display()
                )
            }
            Err(e) => {
                log_info!("Error: {e:?}");
                log_line!("Error: {e:?}")
            }
        }
        self.clear_flags_and_reset_view()
    }

    fn skim_init(&mut self) {
        self.skimer = Some(Skimer::new(Arc::clone(&self.internal_settings.term)));
    }

    /// Replace the tab content with the first result of skim.
    /// It calls skim, reads its output, then update the tab content.
    pub fn skim_output_to_tab(&mut self) -> Result<()> {
        self.skim_init();
        let _ = self._skim_output_to_tab();
        self.drop_skim()
    }

    fn _skim_output_to_tab(&mut self) -> Result<()> {
        let Some(Ok(skimer)) = &self.skimer else {
            return Ok(());
        };
        let skim = skimer.search_filename(&self.current_tab().directory_str());
        let paths: Vec<std::path::PathBuf> = skim
            .iter()
            .map(|s| std::path::PathBuf::from(s.output().to_string()))
            .collect();
        self.menu.flagged.update(paths);
        let Some(output) = skim.first() else {
            return Ok(());
        };
        self._update_tab_from_skim_output(output)
    }

    /// Replace the tab content with the first result of skim.
    /// It calls skim, reads its output, then update the tab content.
    /// The output is splited at `:` since we only care about the path, not the line number.
    pub fn skim_line_output_to_tab(&mut self) -> Result<()> {
        self.skim_init();
        let _ = self._skim_line_output_to_tab();
        self.drop_skim()
    }

    fn _skim_line_output_to_tab(&mut self) -> Result<()> {
        let Some(Ok(skimer)) = &self.skimer else {
            return Ok(());
        };
        let skim = skimer.search_line_in_file(&self.current_tab().directory_str());
        let paths: Vec<std::path::PathBuf> = skim
            .iter()
            .map(|s| std::path::PathBuf::from(s.output().to_string()))
            .collect();
        self.menu.flagged.update(paths);
        let Some(output) = skim.first() else {
            return Ok(());
        };
        self._update_tab_from_skim_line_output(output)
    }

    /// Run a command directly from help.
    /// Search a command in skim, if it's a keybinding, run it directly.
    /// If the result can't be parsed, nothing is done.
    pub fn skim_find_keybinding_and_run(&mut self, help: String) -> Result<()> {
        self.skim_init();
        if let Ok(key) = self._skim_find_keybinding(help) {
            let _ = self.internal_settings.term.send_event(Event::Key(key));
        };
        self.drop_skim()
    }

    fn _skim_find_keybinding(&mut self, help: String) -> Result<tuikit::prelude::Key> {
        let Some(Ok(skimer)) = &mut self.skimer else {
            return Err(anyhow!("Skim isn't initialised"));
        };
        let skim = skimer.search_in_text(&help);
        let Some(output) = skim.first() else {
            return Err(anyhow!("Skim hasn't sent anything"));
        };
        let line = output.output().into_owned();
        let Some(keybind) = line.split(':').next() else {
            return Err(anyhow!("No keybind found"));
        };
        let Some(keyname) = parse_keyname(keybind) else {
            return Err(anyhow!("No keyname found for {keybind}"));
        };
        let Some(key) = from_keyname(&keyname) else {
            return Err(anyhow!("{keyname} isn't a valid Key name."));
        };
        Ok(key)
    }

    fn _update_tab_from_skim_line_output(&mut self, skim_output: &Arc<dyn SkimItem>) -> Result<()> {
        let output_str = skim_output.output().to_string();
        let Some(filename) = output_str.split(':').next() else {
            return Ok(());
        };
        let path = fs::canonicalize(filename)?;
        self._replace_path_by_skim_output(path)
    }

    fn _update_tab_from_skim_output(&mut self, skim_output: &Arc<dyn SkimItem>) -> Result<()> {
        let path = fs::canonicalize(skim_output.output().to_string())?;
        self._replace_path_by_skim_output(path)
    }

    fn _replace_path_by_skim_output(&mut self, path: std::path::PathBuf) -> Result<()> {
        let tab = self.current_tab_mut();
        if path.is_file() {
            let Some(parent) = path.parent() else {
                return Ok(());
            };
            tab.cd(parent)?;
            match tab.display_mode {
                Display::Tree => {
                    tab.tree.go(To::Path(&path));
                }
                Display::Directory => {
                    let index = tab.directory.select_file(&path);
                    tab.go_to_index(index);
                }
                Display::Preview | Display::Flagged => (),
            }
        } else if path.is_dir() {
            tab.cd(&path)?;
        }
        Ok(())
    }

    fn drop_skim(&mut self) -> Result<()> {
        self.skimer = None;
        Ok(())
    }

    /// Update the flagged files depending of the input regex.
    pub fn input_regex(&mut self, char: char) -> Result<()> {
        self.menu.input.insert(char);
        self.select_from_regex()?;
        Ok(())
    }

    /// Flag every file matching a typed regex.
    /// Move to the "first" found match
    pub fn select_from_regex(&mut self) -> Result<()> {
        let input = self.menu.input.string();
        if input.is_empty() {
            return Ok(());
        }
        let paths = match self.current_tab().display_mode {
            Display::Directory => self.tabs[self.index].directory.paths(),
            Display::Tree => self.tabs[self.index].tree.paths(),
            _ => return Ok(()),
        };
        regex_matcher(&input, &paths, &mut self.menu.flagged)?;
        if !self.menu.flagged.is_empty() {
            self.tabs[self.index]
                .go_to_file(self.menu.flagged.selected().context("no selected file")?);
        }
        Ok(())
    }

    /// Open a the selected file with its opener
    pub fn open_selected_file(&mut self) -> Result<()> {
        let path = self.current_tab().current_file()?.path;
        self.open_single_file(&path);
        Ok(())
    }

    pub fn open_single_file(&mut self, path: &std::path::Path) {
        match self.internal_settings.opener.kind(path) {
            Some(Kind::Internal(Internal::NotSupported)) => {
                let _ = self.mount_iso_drive();
            }
            Some(_) => {
                if self.should_this_file_be_opened_in_neovim(path) {
                    self.update_nvim_listen_address();
                    open_in_current_neovim(path, &self.internal_settings.nvim_server);
                } else {
                    let _ = self.internal_settings.opener.open_single(path);
                }
            }
            None => (),
        }
    }

    fn should_this_file_be_opened_in_neovim(&self, path: &std::path::Path) -> bool {
        self.internal_settings.inside_neovim
            && matches!(Extension::matcher(extract_extension(path)), Extension::Text)
    }

    /// Open every flagged file with their respective opener.
    pub fn open_flagged_files(&mut self) -> Result<()> {
        if self
            .menu
            .flagged
            .content()
            .iter()
            .all(|path| self.should_this_file_be_opened_in_neovim(path))
        {
            self.update_nvim_listen_address();
            for path in self.menu.flagged.content().iter() {
                open_in_current_neovim(path, &self.internal_settings.nvim_server);
            }
            Ok(())
        } else {
            self.internal_settings
                .opener
                .open_multiple(self.menu.flagged.content())
        }
    }

    fn ensure_iso_device_is_some(&mut self) -> Result<()> {
        if self.menu.iso_device.is_none() {
            let path = path_to_string(&self.current_tab().current_file()?.path);
            self.menu.iso_device = Some(IsoDevice::from_path(path));
        }
        Ok(())
    }

    /// Mount the currently selected file (which should be an .iso file) to
    /// `/run/media/$CURRENT_USER/fm_iso`
    /// Ask a sudo password first if needed. It should always be the case.
    fn mount_iso_drive(&mut self) -> Result<()> {
        if !self.menu.password_holder.has_sudo() {
            self.ask_password(Some(BlockDeviceAction::MOUNT), PasswordUsage::ISO)?;
        } else {
            self.ensure_iso_device_is_some()?;
            let Some(ref mut iso_device) = self.menu.iso_device else {
                return Ok(());
            };
            if iso_device.mount(&current_username()?, &mut self.menu.password_holder)? {
                log_info!("iso mounter mounted {iso_device:?}");
                log_line!("iso : {}", iso_device.as_string()?);
                let path = iso_device.mountpoints.clone().context("no mount point")?;
                self.current_tab_mut().cd(&path)?;
            };
            self.menu.iso_device = None;
        };

        Ok(())
    }

    /// Currently unused.
    /// Umount an iso device.
    pub fn umount_iso_drive(&mut self) -> Result<()> {
        if let Some(ref mut iso_device) = self.menu.iso_device {
            if !self.menu.password_holder.has_sudo() {
                self.ask_password(Some(BlockDeviceAction::UMOUNT), PasswordUsage::ISO)?;
            } else {
                iso_device.umount(&current_username()?, &mut self.menu.password_holder)?;
            };
        }
        Ok(())
    }

    /// Mount the selected encrypted device. Will ask first for sudo password and
    /// passphrase.
    /// Those passwords are always dropped immediatly after the commands are run.
    pub fn mount_encrypted_drive(&mut self) -> Result<()> {
        let Some(device) = self.menu.encrypted_devices.selected() else {
            return Ok(());
        };
        if device.is_mounted() {
            return Ok(());
        }
        if !self.menu.password_holder.has_sudo() {
            self.ask_password(
                Some(BlockDeviceAction::MOUNT),
                PasswordUsage::CRYPTSETUP(PasswordKind::SUDO),
            )
        } else if !self.menu.password_holder.has_cryptsetup() {
            self.ask_password(
                Some(BlockDeviceAction::MOUNT),
                PasswordUsage::CRYPTSETUP(PasswordKind::CRYPTSETUP),
            )
        } else {
            if let Ok(true) = self
                .menu
                .encrypted_devices
                .mount_selected(&mut self.menu.password_holder)
            {
                self.go_to_encrypted_drive()?;
            }
            Ok(())
        }
    }

    /// Move to the selected crypted device mount point.
    pub fn go_to_encrypted_drive(&mut self) -> Result<()> {
        let Some(path) = self.menu.find_encrypted_drive_mount_point() else {
            return Ok(());
        };
        let tab = self.current_tab_mut();
        tab.cd(&path)?;
        tab.refresh_view()
    }

    /// Unmount the selected device.
    /// Will ask first for a sudo password which is immediatly forgotten.
    pub fn umount_encrypted_drive(&mut self) -> Result<()> {
        let Some(device) = self.menu.encrypted_devices.selected() else {
            return Ok(());
        };
        if !device.is_mounted() {
            return Ok(());
        }
        if !self.menu.password_holder.has_sudo() {
            self.ask_password(
                Some(BlockDeviceAction::UMOUNT),
                PasswordUsage::CRYPTSETUP(PasswordKind::SUDO),
            )
        } else {
            self.menu
                .encrypted_devices
                .umount_selected(&mut self.menu.password_holder)
        }
    }

    pub fn umount_removable(&mut self) -> Result<()> {
        if self.menu.removable_devices.is_empty() {
            return Ok(());
        };
        let device = &mut self.menu.removable_devices.content[self.menu.removable_devices.index];
        if !device.is_mounted() {
            return Ok(());
        }
        if !self.menu.password_holder.has_sudo() && device.is_usb() {
            self.ask_password(Some(BlockDeviceAction::UMOUNT), PasswordUsage::USB)
        } else {
            device.umount_simple(&mut self.menu.password_holder)?;
            Ok(())
        }
    }

    pub fn mount_removable(&mut self) -> Result<()> {
        if self.menu.removable_devices.is_empty() {
            return Ok(());
        };
        let device = &mut self.menu.removable_devices.content[self.menu.removable_devices.index];
        if device.is_mounted() {
            return Ok(());
        }
        if !self.menu.password_holder.has_sudo() && device.is_usb() {
            self.ask_password(Some(BlockDeviceAction::MOUNT), PasswordUsage::USB)
        } else {
            if device.mount_simple(&mut self.menu.password_holder)? {
                self.go_to_removable()?;
            }
            Ok(())
        }
    }

    /// Move to the selected removable device.
    pub fn go_to_removable(&mut self) -> Result<()> {
        let Some(path) = self.menu.find_removable_mount_point() else {
            return Ok(());
        };
        self.current_tab_mut().cd(&path)?;
        self.current_tab_mut().refresh_view()
    }

    /// Reads and parse a shell command. Some arguments may be expanded.
    /// See [`crate::modes::edit::ShellCommandParser`] for more information.
    pub fn parse_shell_command(&mut self) -> Result<bool> {
        let shell_command = self.menu.input.string();
        let mut args = ShellCommandParser::new(&shell_command).compute(self)?;
        log_info!("command {shell_command} args: {args:?}");
        if args_is_empty(&args) {
            self.set_edit_mode(self.index, Edit::Nothing)?;
            return Ok(true);
        }
        let executable = args.remove(0);
        if is_sudo_command(&executable) {
            self.menu.sudo_command = Some(shell_command);
            self.ask_password(None, PasswordUsage::SUDOCOMMAND)?;
            Ok(false)
        } else {
            if !is_program_in_path(&executable) {
                return Ok(true);
            }
            let current_directory = self.current_tab().directory_of_selected()?.to_owned();
            let params: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            if let Ok(output) =
                execute_and_capture_output_with_path(executable, current_directory, &params)
            {
                self.preview_command_output(output, shell_command);
            }
            Ok(true)
        }
    }

    /// Ask for a password of some kind (sudo or device passphrase).
    fn ask_password(
        &mut self,
        encrypted_action: Option<BlockDeviceAction>,
        password_dest: PasswordUsage,
    ) -> Result<()> {
        log_info!("event ask password");
        self.set_edit_mode(
            self.index,
            Edit::InputSimple(InputSimple::Password(encrypted_action, password_dest)),
        )
    }

    /// Attach the typed password to the correct receiver and
    /// execute the command requiring a password.
    pub fn execute_password_command(
        &mut self,
        action: Option<BlockDeviceAction>,
        dest: PasswordUsage,
    ) -> Result<()> {
        let password = self.menu.input.string();
        self.menu.input.reset();
        if matches!(dest, PasswordUsage::CRYPTSETUP(PasswordKind::CRYPTSETUP)) {
            self.menu.password_holder.set_cryptsetup(password)
        } else {
            self.menu.password_holder.set_sudo(password)
        };
        self.reset_edit_mode()?;
        self.dispatch_password(action, dest)
    }

    /// Execute a new mark, saving it to a config file for futher use.
    pub fn marks_new(&mut self, c: char) -> Result<()> {
        let path = self.current_tab_mut().directory.path.clone();
        self.menu.marks.new_mark(c, &path)?;
        {
            let tab: &mut Tab = self.current_tab_mut();
            tab.refresh_view()
        }?;
        self.reset_edit_mode()?;
        self.refresh_status()
    }

    /// Execute a jump to a mark, moving to a valid path.
    /// If the saved path is invalid, it does nothing but reset the view.
    pub fn marks_jump_char(&mut self, c: char) -> Result<()> {
        if let Some(path) = self.menu.marks.get(c) {
            self.current_tab_mut().cd(&path)?;
        }
        self.current_tab_mut().refresh_view()?;
        self.reset_edit_mode()?;
        self.refresh_status()
    }

    /// Recursively delete all flagged files.
    pub fn confirm_delete_files(&mut self) -> Result<()> {
        self.menu.delete_flagged_files()?;
        self.reset_edit_mode()?;
        self.clear_flags_and_reset_view()?;
        self.refresh_status()
    }

    /// Empty the trash folder permanently.
    pub fn confirm_trash_empty(&mut self) -> Result<()> {
        self.menu.trash.empty_trash()?;
        self.reset_edit_mode()?;
        self.clear_flags_and_reset_view()?;
        Ok(())
    }

    /// Ask the new filenames and set the confirmation mode.
    pub fn bulk_ask_filenames(&mut self) -> Result<()> {
        let flagged = self.flagged_in_current_dir();
        let current_path = self.current_tab_path_str();
        self.menu.bulk.ask_filenames(flagged, &current_path)?;
        if let Some(temp_file) = self.menu.bulk.temp_file() {
            self.open_single_file(&temp_file);
            self.menu.bulk.watch_in_thread()?;
        }
        Ok(())
    }

    pub fn bulk_execute(&mut self) -> Result<()> {
        self.menu.bulk.get_new_names()?;
        self.set_edit_mode(
            self.index,
            Edit::NeedConfirmation(NeedConfirmation::BulkAction),
        )?;
        Ok(())
    }

    /// Execute the bulk action.
    pub fn confirm_bulk_action(&mut self) -> Result<()> {
        if let (Some(paths), Some(create)) = self.menu.bulk.execute()? {
            self.menu.flagged.update(paths);
            self.menu.flagged.extend(create);
        } else {
            self.menu.flagged.clear();
        };
        self.reset_edit_mode()?;
        self.reset_tabs_view()?;
        Ok(())
    }

    fn run_sudo_command(&mut self) -> Result<()> {
        self.set_edit_mode(self.index, Edit::Nothing)?;
        reset_sudo_faillock()?;
        let Some(sudo_command) = self.menu.sudo_command.to_owned() else {
            return self.menu.clear_sudo_attributes();
        };
        let args = ShellCommandParser::new(&sudo_command).compute(self)?;
        if args.is_empty() {
            return self.menu.clear_sudo_attributes();
        }
        let (_, output, _) = execute_sudo_command_with_password(
            &args[1..],
            self.menu
                .password_holder
                .sudo()
                .as_ref()
                .context("sudo password isn't set")?,
            self.current_tab().directory_of_selected()?,
        )?;
        self.menu.clear_sudo_attributes()?;
        self.preview_command_output(output, sudo_command.to_owned());
        // self.refresh_status()
        Ok(())
    }

    /// Dispatch the known password depending of which component set
    /// the `PasswordUsage`.
    pub fn dispatch_password(
        &mut self,
        action: Option<BlockDeviceAction>,
        dest: PasswordUsage,
    ) -> Result<()> {
        match dest {
            PasswordUsage::USB => match action {
                Some(BlockDeviceAction::MOUNT) => self.mount_removable(),
                Some(BlockDeviceAction::UMOUNT) => self.umount_removable(),
                None => Ok(()),
            },
            PasswordUsage::ISO => match action {
                Some(BlockDeviceAction::MOUNT) => self.mount_iso_drive(),
                Some(BlockDeviceAction::UMOUNT) => self.umount_iso_drive(),
                None => Ok(()),
            },
            PasswordUsage::CRYPTSETUP(_) => match action {
                Some(BlockDeviceAction::MOUNT) => self.mount_encrypted_drive(),
                Some(BlockDeviceAction::UMOUNT) => self.umount_encrypted_drive(),
                None => Ok(()),
            },
            PasswordUsage::SUDOCOMMAND => self.run_sudo_command(),
        }
    }

    /// Set the display to preview a command output
    pub fn preview_command_output(&mut self, output: String, command: String) {
        log_info!("output {output}");
        let _ = self.reset_edit_mode();
        self.current_tab_mut().set_display_mode(Display::Preview);
        let preview = Preview::cli_info(&output, command);
        self.current_tab_mut().window.reset(preview.len());
        // TODO ! help & command
        // self.current_tab_mut().preview = preview;
    }

    /// Set the nvim listen address from what the user typed.
    pub fn update_nvim_listen_address(&mut self) {
        self.internal_settings.update_nvim_listen_address()
    }

    /// Execute a command requiring a confirmation (Delete, Move or Copy).
    /// The action is only executed if the user typed the char `y`
    pub fn confirm(&mut self, c: char, confirmed_action: NeedConfirmation) -> Result<()> {
        if c == 'y' {
            let _ = self.match_confirmed_mode(confirmed_action);
        }
        self.reset_edit_mode()?;
        self.current_tab_mut().refresh_view()?;

        Ok(())
    }

    /// Execute a `NeedConfirmation` action (delete, move, copy, empty trash)
    fn match_confirmed_mode(&mut self, confirmed_action: NeedConfirmation) -> Result<()> {
        match confirmed_action {
            NeedConfirmation::Delete => self.confirm_delete_files(),
            NeedConfirmation::Move => self.cut_or_copy_flagged_files(CopyMove::Move),
            NeedConfirmation::Copy => self.cut_or_copy_flagged_files(CopyMove::Copy),
            NeedConfirmation::EmptyTrash => self.confirm_trash_empty(),
            NeedConfirmation::BulkAction => self.confirm_bulk_action(),
        }
    }

    /// Execute an action when the header line was clicked.
    pub fn header_action(&mut self, col: u16, binds: &Bindings) -> Result<()> {
        let is_right = !self.focus.is_left();
        match self.current_tab().display_mode {
            Display::Preview => Ok(()),
            Display::Flagged => FlaggedHeader::new(self)?
                .action(col as usize, is_right)
                .matcher(self, binds),
            _ => Header::new(self, self.current_tab())?
                .action(col as usize, is_right)
                .matcher(self, binds),
        }
    }

    /// Execute an action when the footer line was clicked.
    pub fn footer_action(&mut self, col: u16, binds: &Bindings) -> Result<()> {
        log_info!("footer clicked col {col}");
        let is_right = self.index == 1;
        let action = match self.current_tab().display_mode {
            Display::Preview => return Ok(()),
            Display::Tree | Display::Directory => {
                let footer = Footer::new(self, self.current_tab())?;
                footer.action(col as usize, is_right).to_owned()
            }
            Display::Flagged => {
                let footer = FlaggedFooter::new(self)?;
                footer.action(col as usize, is_right).to_owned()
            }
        };
        log_info!("action: {action}");
        action.matcher(self, binds)
    }

    /// Change permission of the flagged files.
    /// Once the user has typed an octal permission like 754, it's applied to
    /// the file.
    /// Nothing is done if the user typed nothing or an invalid permission like
    /// 955.
    pub fn chmod(&mut self) -> Result<()> {
        if self.menu.input.is_empty() || self.menu.flagged.is_empty() {
            return Ok(());
        }
        let input_permission = &self.menu.input.string();
        Permissions::set_permissions_of_flagged(input_permission, &mut self.menu.flagged)?;
        self.reset_tabs_view()
    }

    /// Enter the chmod mode where user can chmod a file.
    pub fn set_mode_chmod(&mut self) -> Result<()> {
        if self.current_tab_mut().directory.is_empty() {
            return Ok(());
        }
        if self.menu.flagged.is_empty() {
            self.toggle_flag_for_selected();
        }
        self.set_edit_mode(self.index, Edit::InputSimple(InputSimple::Chmod))
    }

    /// Execute a custom event on the selected file
    pub fn run_custom_command(&mut self, string: &str) -> Result<()> {
        log_info!("custom {string}");
        let parser = ShellCommandParser::new(string);
        let mut args = parser.compute(self)?;
        let command = args.remove(0);
        let args: Vec<&str> = args.iter().map(|s| &**s).collect();
        let output = execute_and_capture_output_without_check(command, &args)?;
        log_info!("output {output}");
        Ok(())
    }

    pub fn fuzzy_flags(&mut self) -> Result<()> {
        self.current_tab_mut().set_display_mode(Display::Flagged);
        Ok(())
    }

    pub fn sort(&mut self, c: char) -> Result<()> {
        self.current_tab_mut().sort(c)?;
        self.menu.reset();
        self.set_height_for_edit_mode(self.index, Edit::Nothing)?;
        self.tabs[self.index].edit_mode = Edit::Nothing;
        let len = self.menu.len(Edit::Nothing);
        let height = self.second_window_height()?;
        self.menu.window = ContentWindow::new(len, height);
        self.focus = self.focus.to_parent();
        Ok(())
    }

    /// The width of a displayed canvas.
    pub fn canvas_width(&self) -> Result<usize> {
        let full_width = self.internal_settings.term_size()?.0;
        if self.display_settings.dual() && full_width >= MIN_WIDTH_FOR_DUAL_PANE {
            Ok(full_width / 2)
        } else {
            Ok(full_width)
        }
    }

    /// Set a new filter.
    /// Doesn't reset the input.
    pub fn set_filter(&mut self) -> Result<()> {
        let filter = FilterKind::from_input(&self.menu.input.string());
        self.current_tab_mut().settings.set_filter(filter);
        // ugly hack to please borrow checker :(
        self.tabs[self.index].directory.reset_files(
            &self.tabs[self.index].settings,
            &self.tabs[self.index].users,
        )?;
        if let Display::Tree = self.current_tab().display_mode {
            self.current_tab_mut().make_tree(None)?;
        }
        let len = self.current_tab().directory.content.len();
        self.current_tab_mut().window.reset(len);
        Ok(())
    }

    /// input the typed char and update the filterkind.
    pub fn input_filter(&mut self, c: char) -> Result<()> {
        self.menu.input_insert(c)?;
        self.set_filter()
    }

    fn update_preview_len(&mut self) {
        let Some(previewed_doc) = &self.tabs[self.index].previewed_doc else {
            return;
        };
        let Ok(preview_holder) = self.preview_holder.lock() else {
            return;
        };
        let Some(preview) = preview_holder.get(Path::new(previewed_doc)) else {
            return;
        };
        self.tabs[self.index].preview_len = preview.len();
    }

    /// Move the preview to the top
    pub fn preview_go_top(&mut self) {
        self.update_preview_len();
        self.tabs[self.index].window.scroll_to(0)
    }

    /// Move the preview to the bottom
    pub fn preview_go_bottom(&mut self) {
        self.update_preview_len();
        self.tabs[self.index].window.scroll_to(
            self.tabs[self.index]
                .preview_len
                .checked_sub(1)
                .unwrap_or_default(),
        )
    }

    /// Move 30 lines up or an image in Ueberzug.
    pub fn preview_page_up(&mut self) {
        // match &mut self.preview {
        // Preview::Ueberzug(ref mut image) => image.up_one_row(),
        // _ => {
        self.update_preview_len();
        if self.tabs[self.index].window.top > 0 {
            let skip = min(self.tabs[self.index].window.top, 30);
            self.tabs[self.index].window.bottom -= skip;
            self.tabs[self.index].window.top -= skip;
        }
        //     }
        // }
    }

    /// Move down 30 rows except for Ueberzug where it moves 1 image down
    pub fn preview_page_down(&mut self) {
        // match &mut self.preview {
        //     Preview::Ueberzug(ref mut image) => image.down_one_row(),
        //     _ => {
        self.update_preview_len();
        if self.tabs[self.index].window.bottom < self.tabs[self.index].preview_len {
            let skip = min(
                self.tabs[self.index].preview_len - self.tabs[self.index].window.bottom,
                30,
            );
            self.tabs[self.index].window.bottom += skip;
            self.tabs[self.index].window.top += skip;
            //     }
            // }
        }
    }
}

fn parse_keyname(keyname: &str) -> Option<String> {
    let mut split = keyname.split('(');
    let Some(mutator) = split.next() else {
        return None;
    };
    let mut mutator = mutator.to_lowercase();
    let Some(param) = split.next() else {
        return Some(mutator);
    };
    let mut param = param.trim().to_owned();
    mutator = mutator.replace("char", "");
    param = param.replace([')', '\''], "");
    if param.chars().all(char::is_uppercase) {
        if mutator.is_empty() {
            mutator = "shift".to_owned();
        } else {
            mutator = format!("{mutator}-shift");
        }
    }

    if mutator.is_empty() {
        Some(param)
    } else {
        Some(format!("{mutator}-{param}"))
    }
}
