use std::cmp::min;
use std::path::PathBuf;
use std::sync::{Arc, MutexGuard};

use anyhow::{Context, Result};
use tuikit::attr::{Attr, Color};
use tuikit::prelude::*;
use tuikit::term::Term;

use crate::app::Footer;
use crate::app::Status;
use crate::app::Tab;
use crate::app::{ClickableLine, ClickableString, FlaggedFooter, FlaggedHeader};
use crate::app::{Header, PreviewHeader};
use crate::common::path_to_string;
use crate::common::ENCRYPTED_DEVICE_BINDS;
use crate::config::{ColorG, Gradient, MENU_COLORS};
use crate::io::read_last_log_line;
use crate::io::ModeFormat;
use crate::log_info;
use crate::modes::BinaryContent;
use crate::modes::ColoredText;
use crate::modes::Content;
use crate::modes::ContentWindow;
use crate::modes::Display as DisplayMode;
use crate::modes::Edit;
use crate::modes::FileInfo;
use crate::modes::HLContent;
use crate::modes::InputSimple;
use crate::modes::LineDisplay;
use crate::modes::MountRepr;
use crate::modes::Navigate;
use crate::modes::NeedConfirmation;
use crate::modes::Preview;
use crate::modes::Selectable;
use crate::modes::Trash;
use crate::modes::TreeLineBuilder;
use crate::modes::TreePreview;
use crate::modes::Ueberzug;
use crate::modes::Window;
use crate::modes::{fileinfo_attr, MarkAction};
use crate::modes::{parse_input_mode, SecondLine};

trait ClearLine {
    fn clear_line(&mut self, row: usize) -> Result<()>;
}

impl ClearLine for dyn Canvas + '_ {
    fn clear_line(&mut self, row: usize) -> Result<()> {
        let (width, _) = self.size()?;
        self.print(row, 0, &" ".repeat(width))?;
        Ok(())
    }
}

/// Iter over the content, returning a triplet of `(index, line, attr)`.
macro_rules! enumerated_colored_iter {
    ($t:ident) => {
        std::iter::zip(
            $t.iter().enumerate(),
            Gradient::new(
                ColorG::from_tuikit(
                    MENU_COLORS
                        .get()
                        .expect("Menu colors should be set")
                        .first
                        .fg,
                )
                .unwrap_or_default(),
                ColorG::from_tuikit(
                    MENU_COLORS
                        .get()
                        .expect("Menu colors should be set")
                        .palette_3
                        .fg,
                )
                .unwrap_or_default(),
                $t.len(),
            )
            .gradient()
            .map(|color| color_to_attr(color)),
        )
        .map(|((index, line), attr)| (index, line, attr))
    };
}
/// Draw every line of the preview
macro_rules! impl_preview {
    ($text:ident, $tab:ident, $length:ident, $canvas:ident, $line_number_width:ident, $window:ident, $height:ident) => {
        for (i, line) in (*$text).window($window.top, $window.bottom, $length) {
            let row = calc_line_row(i, $window);
            if row > $height {
                break;
            }
            $canvas.print(row, $line_number_width + 3, line)?;
        }
    };
}

/// At least 120 chars width to display 2 tabs.
pub const MIN_WIDTH_FOR_DUAL_PANE: usize = 120;

enum TabPosition {
    Left,
    Right,
}

/// Bunch of attributes describing the state of a main window
/// relatively to other windows
struct WinMainAttributes {
    /// horizontal position, in cells
    x_position: usize,
    /// is this the first (left) or second (right) window ?
    tab_position: TabPosition,
    /// is this tab selected ?
    is_selected: bool,
    /// is there a secondary window ?
    has_window_below: bool,
}

impl WinMainAttributes {
    fn new(
        x_position: usize,
        tab_position: TabPosition,
        is_selected: bool,
        has_window_below: bool,
    ) -> Self {
        Self {
            x_position,
            tab_position,
            is_selected,
            has_window_below,
        }
    }

    fn is_right(&self) -> bool {
        matches!(self.tab_position, TabPosition::Right)
    }
}

pub trait Height: Canvas {
    fn height(&self) -> Result<usize> {
        Ok(self.size()?.1)
    }
}

impl Height for dyn Canvas + '_ {}

struct WinMain<'a> {
    status: &'a Status,
    tab: &'a Tab,
    attributes: WinMainAttributes,
}

impl<'a> Draw for WinMain<'a> {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        if self.status.display_settings.dual()
            && self.is_right()
            && self.status.display_settings.preview()
        {
            self.draw_preview_as_second_pane(canvas)?;
            return Ok(());
        }
        WinMainHeader::new(self.status, self.tab, self.attributes.is_selected)?.draw(canvas)?;
        self.draw_content(canvas)?;
        WinMainFooter::new(self.status, self.tab, self.attributes.is_selected)?.draw(canvas)?;
        Ok(())
    }
}

impl<'a> Widget for WinMain<'a> {}

impl<'a> WinMain<'a> {
    fn new(status: &'a Status, index: usize, attributes: WinMainAttributes) -> Self {
        Self {
            status,
            tab: &status.tabs[index],
            attributes,
        }
    }

    fn is_right(&self) -> bool {
        self.attributes.is_right()
    }

    fn draw_content(&self, canvas: &mut dyn Canvas) -> Result<Option<usize>> {
        self.draw_copy_progress_bar(canvas)?;
        match &self.tab.display_mode {
            DisplayMode::Directory => self.draw_files(canvas),
            DisplayMode::Tree => self.draw_tree(canvas),
            DisplayMode::Preview => self.draw_preview(self.tab, &self.tab.window, canvas),
            DisplayMode::Flagged => self.draw_fagged(canvas),
        }
    }

    /// Display a copy progress bar on the left tab.
    /// Nothing is drawn if there's no copy atm.
    /// If the copy file queue has length > 1, we also display its size.
    fn draw_copy_progress_bar(&self, canvas: &mut dyn Canvas) -> Result<usize> {
        if self.is_right() {
            return Ok(0);
        }
        let Some(copy_progress) = &self.status.internal_settings.in_mem_progress else {
            return Ok(0);
        };
        let progress_bar = copy_progress.contents();
        let nb_copy_left = self.status.internal_settings.copy_file_queue.len();
        let content = if nb_copy_left <= 1 {
            progress_bar
        } else {
            format!("{progress_bar}     -     1 of {nb}", nb = nb_copy_left)
        };
        Ok(canvas.print_with_attr(
            1,
            2,
            &content,
            MENU_COLORS
                .get()
                .expect("Menu colors should be set")
                .palette_4,
        )?)
    }

    /// Displays the current directory content, one line per item like in
    /// `ls -l`.
    ///
    /// Only the files around the selected one are displayed.
    /// We reverse the attributes of the selected one, underline the flagged files.
    /// When we display a simpler version, the second line is used to display the
    /// metadata of the selected file.
    fn draw_files(&self, canvas: &mut dyn Canvas) -> Result<Option<usize>> {
        let _ = WinMainSecondLine::new(self.status, self.tab).draw(canvas);
        self.draw_files_content(canvas)?;
        if !self.attributes.has_window_below && !self.attributes.is_right() {
            let _ = LogLine {}.draw(canvas);
        }
        Ok(None)
    }

    fn draw_files_content(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let len = self.tab.directory.content.len();
        let group_size: usize;
        let owner_size: usize;
        if self.status.display_settings.metadata() {
            group_size = self.tab.directory.group_column_width();
            owner_size = self.tab.directory.owner_column_width();
        } else {
            group_size = 0;
            owner_size = 0;
        }

        let height = canvas.height()?;
        for (index, file) in self
            .tab
            .directory
            .enumerate()
            .skip(self.tab.window.top)
            .take(min(len, self.tab.window.bottom))
        {
            self.draw_files_line(canvas, group_size, owner_size, index, file, height)?;
        }
        Ok(())
    }

    fn draw_files_line(
        &self,
        canvas: &mut dyn Canvas,
        group_size: usize,
        owner_size: usize,
        index: usize,
        file: &FileInfo,
        height: usize,
    ) -> Result<()> {
        let row = index + ContentWindow::WINDOW_MARGIN_TOP - self.tab.window.top;
        if row > height {
            return Ok(());
        }
        let mut attr = fileinfo_attr(file);
        if index == self.tab.directory.index {
            attr.effect |= Effect::REVERSE;
        }
        let content = self.format_file_content(file, owner_size, group_size)?;
        self.print_as_flagged(canvas, row, &file.path, &mut attr)?;
        let col = if self.status.menu.flagged.contains(&file.path) {
            2
        } else {
            1
        };
        canvas.print_with_attr(row, col, &content, attr)?;
        Ok(())
    }

    fn format_file_content(
        &self,
        file: &FileInfo,
        owner_size: usize,
        group_size: usize,
    ) -> Result<String> {
        if self.status.display_settings.metadata() {
            file.format(owner_size, group_size)
        } else {
            file.format_simple()
        }
    }

    fn print_as_flagged(
        &self,
        canvas: &mut dyn Canvas,
        row: usize,
        path: &std::path::Path,
        attr: &mut Attr,
    ) -> Result<()> {
        if self.status.menu.flagged.contains(path) {
            attr.effect |= Effect::BOLD;
            canvas.print_with_attr(
                row,
                0,
                "█",
                MENU_COLORS.get().expect("Menu colors should be set").second,
            )?;
        }
        Ok(())
    }

    fn draw_tree(&self, canvas: &mut dyn Canvas) -> Result<Option<usize>> {
        let _ = WinMainSecondLine::new(self.status, self.tab).draw(canvas);
        let selected_index = self.draw_tree_content(canvas)?;
        Ok(Some(selected_index))
    }

    fn draw_tree_content(&self, canvas: &mut dyn Canvas) -> Result<usize> {
        let left_margin = 1;
        let height = canvas.height()?;
        let length = self.tab.tree.displayable().lines().len();

        for (index, content_line) in self
            .tab
            .tree
            .displayable()
            .lines()
            .iter()
            .enumerate()
            .skip(self.tab.window.top)
            .take(min(length, self.tab.window.bottom + 1))
        {
            self.draw_tree_line(
                canvas,
                content_line,
                TreeLinePosition {
                    left_margin,
                    top: self.tab.window.top,
                    index,
                    height,
                },
                self.status.display_settings.metadata(),
            )?;
        }
        Ok(self.tab.tree.displayable().index())
    }

    fn draw_tree_line(
        &self,
        canvas: &mut dyn Canvas,
        tree_line_maker: &TreeLineBuilder,
        position_param: TreeLinePosition,
        display_medatadata: bool,
    ) -> Result<()> {
        let (left_margin, top, index, height) = position_param.export();
        let row = index + ContentWindow::WINDOW_MARGIN_TOP - top;
        if row > height {
            return Ok(());
        }

        let s_prefix = tree_line_maker.prefix();
        let mut attr = tree_line_maker.attr();
        let path = tree_line_maker.path();

        self.print_as_flagged(canvas, row, path, &mut attr)?;

        let col_metadata = if display_medatadata {
            let s_metadata = tree_line_maker.metadata();
            canvas.print_with_attr(row, left_margin, s_metadata, attr)?
        } else {
            0
        };

        let offset = if index == 0 { 2 } else { 1 };
        let col_tree_prefix = canvas.print(row, left_margin + col_metadata + offset, s_prefix)?;

        let flagged_offset = if self.status.menu.flagged.contains(path) {
            1
        } else {
            0
        };
        canvas.print_with_attr(
            row,
            left_margin + col_metadata + col_tree_prefix + offset + flagged_offset,
            &tree_line_maker.filename(),
            attr,
        )?;
        Ok(())
    }

    fn draw_line_number(
        row_position_in_canvas: usize,
        line_number_to_print: usize,
        canvas: &mut dyn Canvas,
    ) -> Result<usize> {
        Ok(canvas.print_with_attr(
            row_position_in_canvas,
            0,
            &line_number_to_print.to_string(),
            MENU_COLORS.get().expect("Menu colors should be set").first,
        )?)
    }

    /// Display a scrollable preview of a file.
    /// Multiple modes are supported :
    /// if the filename extension is recognized, the preview is highlighted,
    /// if the file content is recognized as binary, an hex dump is previewed with 16 bytes lines,
    /// else the content is supposed to be text and shown as such.
    /// It may fail to recognize some usual extensions, notably `.toml`.
    /// It may fail to recognize small files (< 1024 bytes).
    fn draw_preview(
        &self,
        tab: &Tab,
        window: &ContentWindow,
        canvas: &mut dyn Canvas,
    ) -> Result<Option<usize>> {
        let length = tab.preview.len();
        let line_number_width = length.to_string().len();
        let height = canvas.height()?;
        match &tab.preview {
            Preview::Syntaxed(syntaxed) => {
                self.draw_syntaxed(syntaxed, length, canvas, line_number_width, window)?
            }
            Preview::Binary(bin) => self.draw_binary(bin, length, canvas, window)?,
            Preview::Ueberzug(image) => self.draw_ueberzug(image, canvas)?,
            Preview::Tree(tree_preview) => self.draw_tree_preview(tree_preview, window, canvas)?,
            Preview::ColoredText(colored_text) => {
                self.draw_colored_text(colored_text, length, canvas, window)?
            }
            Preview::Archive(text) => {
                impl_preview!(text, tab, length, canvas, line_number_width, window, height)
            }
            Preview::Media(text) => {
                impl_preview!(text, tab, length, canvas, line_number_width, window, height)
            }
            Preview::Text(text) => {
                impl_preview!(text, tab, length, canvas, line_number_width, window, height)
            }
            Preview::Iso(text) => {
                impl_preview!(text, tab, length, canvas, line_number_width, window, height)
            }
            Preview::Socket(text) => {
                impl_preview!(text, tab, length, canvas, line_number_width, window, height)
            }
            Preview::BlockDevice(text) => {
                impl_preview!(text, tab, length, canvas, line_number_width, window, height)
            }
            Preview::FifoCharDevice(text) => {
                impl_preview!(text, tab, length, canvas, line_number_width, window, height)
            }
            Preview::Torrent(text) => {
                impl_preview!(text, tab, length, canvas, line_number_width, window, height)
            }

            Preview::Empty => (),
        }
        Ok(None)
    }

    fn draw_syntaxed(
        &self,
        syntaxed: &HLContent,
        length: usize,
        canvas: &mut dyn Canvas,
        line_number_width: usize,
        window: &ContentWindow,
    ) -> Result<()> {
        for (i, vec_line) in (*syntaxed).window(window.top, window.bottom, length) {
            let row_position = calc_line_row(i, window);
            Self::draw_line_number(row_position, i + 1, canvas)?;
            for token in vec_line.iter() {
                token.print(canvas, row_position, line_number_width)?;
            }
        }
        Ok(())
    }

    fn draw_binary(
        &self,
        bin: &BinaryContent,
        length: usize,
        canvas: &mut dyn Canvas,
        window: &ContentWindow,
    ) -> Result<()> {
        let height = canvas.height()?;
        let line_number_width_hex = format!("{:x}", bin.len() * 16).len();

        for (i, line) in (*bin).window(window.top, window.bottom, length) {
            let row = calc_line_row(i, window);
            if row > height {
                break;
            }
            canvas.print_with_attr(
                row,
                0,
                &format_line_nr_hex(i + 1 + window.top, line_number_width_hex),
                MENU_COLORS.get().expect("Menu colors should be set").first,
            )?;
            line.print_bytes(canvas, row, line_number_width_hex + 1);
            line.print_ascii(canvas, row, line_number_width_hex + 43);
        }
        Ok(())
    }

    fn draw_ueberzug(&self, image: &Ueberzug, canvas: &mut dyn Canvas) -> Result<()> {
        let (width, height) = canvas.size()?;
        image.match_index()?;
        image.ueberzug(
            self.attributes.x_position as u16 + 2,
            3,
            width as u16 - 2,
            height as u16 - 2,
        );
        Ok(())
    }

    fn draw_tree_preview(
        &self,
        tree_preview: &TreePreview,
        window: &ContentWindow,
        canvas: &mut dyn Canvas,
    ) -> Result<()> {
        let height = canvas.height()?;
        let tree_content = tree_preview.tree.displayable();
        let content = tree_content.lines();
        let length = content.len();

        for (index, content_line) in
            tree_preview
                .tree
                .displayable()
                .window(window.top, window.bottom, length)
        {
            self.draw_tree_line(
                canvas,
                content_line,
                TreeLinePosition {
                    left_margin: 0,
                    top: window.top,
                    index,
                    height,
                },
                false,
            )?;
        }
        Ok(())
    }

    fn draw_colored_text(
        &self,
        colored_text: &ColoredText,
        length: usize,
        canvas: &mut dyn Canvas,
        window: &ContentWindow,
    ) -> Result<()> {
        let height = canvas.height()?;
        for (i, line) in colored_text.window(window.top, window.bottom, length) {
            let row = calc_line_row(i, window);
            if row > height {
                break;
            }
            let mut col = 3;
            for (chr, attr) in skim::AnsiString::parse(line).iter() {
                col += canvas.print_with_attr(row, col, &chr.to_string(), attr)?;
            }
        }
        Ok(())
    }

    fn draw_preview_as_second_pane(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let tab = &self.status.tabs[1];
        self.draw_preview(tab, &tab.window, canvas)?;
        let (width, _) = canvas.size()?;
        draw_clickable_strings(
            0,
            0,
            &PreviewHeader::default_preview(self.status, tab, width),
            canvas,
            false,
        )?;
        Ok(())
    }

    fn draw_fagged(&self, canvas: &mut dyn Canvas) -> Result<Option<usize>> {
        let window = &self.status.menu.flagged.window;
        for (index, path) in self
            .status
            .menu
            .flagged
            .content
            .iter()
            .enumerate()
            .skip(window.top)
            .take(min(canvas.height()?, window.bottom + 1))
        {
            let fileinfo = FileInfo::new(path, &self.tab.users)?;
            let mut attr = fileinfo_attr(&fileinfo);
            if index == self.status.menu.flagged.index {
                attr.effect |= Effect::REVERSE;
            }
            let row = index + 3 - window.top;
            canvas.print_with_attr(row, 1, &fileinfo.path.to_string_lossy(), attr)?;
        }
        if let Some(selected) = self.status.menu.flagged.selected() {
            let fileinfo = FileInfo::new(selected, &self.tab.users)?;
            canvas.print_with_attr(1, 1, &fileinfo.format(6, 6)?, fileinfo_attr(&fileinfo))?;
        };
        Ok(None)
    }
}

struct TreeLinePosition {
    left_margin: usize,
    top: usize,
    index: usize,
    height: usize,
}

impl TreeLinePosition {
    /// left_margin, top, index, height
    fn export(&self) -> (usize, usize, usize, usize) {
        (self.left_margin, self.top, self.index, self.height)
    }
}

struct WinMainHeader<'a> {
    status: &'a Status,
    tab: &'a Tab,
    is_selected: bool,
}

impl<'a> Draw for WinMainHeader<'a> {
    /// Display the top line on terminal.
    /// Its content depends on the mode.
    /// In normal mode we display the path and number of files.
    /// When a confirmation is needed we ask the user to input `'y'` or
    /// something else.
    /// Returns the result of the number of printed chars.
    /// The colors are reversed when the tab is selected. It gives a visual indication of where he is.
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        let (width, _) = canvas.size()?;
        let content = match self.tab.display_mode {
            DisplayMode::Preview => PreviewHeader::elems(self.status, self.tab, width),
            DisplayMode::Flagged => FlaggedHeader::new(self.status)?.elems().to_vec(),
            _ => Header::new(self.status, self.tab)?.elems().to_owned(),
        };
        draw_clickable_strings(0, 0, &content, canvas, self.is_selected)?;
        Ok(())
    }
}

impl<'a> WinMainHeader<'a> {
    fn new(status: &'a Status, tab: &'a Tab, is_selected: bool) -> Result<Self> {
        Ok(Self {
            status,
            tab,
            is_selected,
        })
    }
}

#[derive(Default)]
struct WinMainSecondLine {
    content: Option<String>,
    attr: Option<Attr>,
}

impl Draw for WinMainSecondLine {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        match (&self.content, &self.attr) {
            (Some(content), Some(attr)) => canvas.print_with_attr(1, 1, content, *attr)?,
            _ => 0,
        };
        Ok(())
    }
}

impl WinMainSecondLine {
    fn new(status: &Status, tab: &Tab) -> Self {
        if matches!(tab.display_mode, DisplayMode::Preview) || status.display_settings.metadata() {
            return Self::default();
        };
        if let Ok(file) = tab.current_file() {
            Self::second_line_detailed(&file)
        } else {
            Self::default()
        }
    }

    fn second_line_detailed(file: &FileInfo) -> Self {
        let owner_size = file.owner.len();
        let group_size = file.group.len();
        let mut attr = fileinfo_attr(file);
        attr.effect ^= Effect::REVERSE;

        Self {
            content: Some(file.format(owner_size, group_size).unwrap_or_default()),
            attr: Some(attr),
        }
    }
}

struct LogLine;

impl Draw for LogLine {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        let height = canvas.height()?;
        canvas.print_with_attr(
            height - 2,
            4,
            &read_last_log_line(),
            MENU_COLORS.get().expect("Menu colors should be set").second,
        )?;
        Ok(())
    }
}

struct WinMainFooter<'a> {
    status: &'a Status,
    tab: &'a Tab,
    is_selected: bool,
}

impl<'a> Draw for WinMainFooter<'a> {
    /// Display the top line on terminal.
    /// Its content depends on the mode.
    /// In normal mode we display the path and number of files.
    /// When a confirmation is needed we ask the user to input `'y'` or
    /// something else.
    /// Returns the result of the number of printed chars.
    /// The colors are reversed when the tab is selected. It gives a visual indication of where he is.
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        let (width, height) = canvas.size()?;
        let content = match self.tab.display_mode {
            DisplayMode::Preview => vec![],
            DisplayMode::Flagged => FlaggedFooter::new(self.status)?.elems().to_owned(),
            _ => Footer::new(self.status, self.tab)?.elems().to_owned(),
        };
        let mut attr = MENU_COLORS.get().expect("Menu colors should be set").first;
        let last_index = (content.len().saturating_sub(1))
            % MENU_COLORS
                .get()
                .expect("Menu colors should be set")
                .palette_size();
        let mut background = MENU_COLORS
            .get()
            .expect("Menu colors should be set")
            .palette()[last_index];
        if self.is_selected {
            attr.effect |= Effect::REVERSE;
            background.effect |= Effect::REVERSE;
        };
        canvas.print_with_attr(height - 1, 0, &" ".repeat(width), background)?;
        draw_clickable_strings(height - 1, 0, &content, canvas, self.is_selected)?;
        Ok(())
    }
}

impl<'a> WinMainFooter<'a> {
    fn new(status: &'a Status, tab: &'a Tab, is_selected: bool) -> Result<Self> {
        Ok(Self {
            status,
            tab,
            is_selected,
        })
    }
}

struct WinSecondary<'a> {
    status: &'a Status,
    tab: &'a Tab,
}

impl<'a> Draw for WinSecondary<'a> {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        self.draw_cursor(canvas)?;
        WinSecondaryFirstLine::new(self.status).draw(canvas)?;
        self.draw_second_line(canvas)?;
        match self.tab.edit_mode {
            Edit::Navigate(mode) => self.draw_navigate(mode, canvas),
            Edit::NeedConfirmation(mode) => self.draw_confirm(mode, canvas),
            Edit::InputCompleted(_) => self.draw_completion(canvas),
            Edit::InputSimple(mode) => Self::draw_static_lines(mode.lines(), canvas),
            _ => return Ok(()),
        }?;
        self.draw_binds_per_mode(canvas, self.tab.edit_mode)?;
        Ok(())
    }
}

impl<'a> WinSecondary<'a> {
    fn new(status: &'a Status, index: usize) -> Self {
        Self {
            status,
            tab: &status.tabs[index],
        }
    }

    fn draw_second_line(&self, canvas: &mut dyn Canvas) -> Result<()> {
        match self.tab.edit_mode {
            Edit::InputSimple(InputSimple::Chmod) => {
                let mode_parsed = parse_input_mode(&self.status.menu.input.string());
                let mut col = 11;
                for (text, is_valid) in &mode_parsed {
                    let attr = if *is_valid {
                        MENU_COLORS.get().expect("Menu colors should be set").first
                    } else {
                        MENU_COLORS.get().expect("Menu colors should be set").second
                    };
                    col += 1 + canvas.print_with_attr(1, col, text, attr)?;
                }
            }
            edit => {
                canvas.print_with_attr(
                    1,
                    2,
                    edit.second_line(),
                    MENU_COLORS.get().expect("Menu colors should be set").second,
                )?;
            }
        }
        Ok(())
    }

    fn draw_binds_per_mode(&self, canvas: &mut dyn Canvas, mode: Edit) -> Result<()> {
        let height = canvas.height()?;
        canvas.clear_line(height - 1)?;
        canvas.print_with_attr(
            height - 1,
            2,
            mode.binds_per_mode(),
            MENU_COLORS.get().expect("Menu colors should be set").second,
        )?;
        Ok(())
    }

    /// Display the possible completion items. The currently selected one is
    /// reversed.
    fn draw_completion(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let content = &self.status.menu.completion.proposals;
        let (top, bottom) = (self.status.menu.window.top, self.status.menu.window.bottom);
        let len = content.len();
        for (row, candidate, attr) in enumerated_colored_iter!(content)
            .skip(top)
            .take(min(bottom, len))
        {
            let attr = self.status.menu.completion.attr(row, &attr);
            Self::draw_content_line(canvas, row + 1 - top, candidate, attr)?;
        }
        Ok(())
    }

    fn draw_static_lines(lines: &[&str], canvas: &mut dyn Canvas) -> Result<()> {
        for (row, line, attr) in enumerated_colored_iter!(lines) {
            Self::draw_content_line(canvas, row, line, attr)?;
        }
        Ok(())
    }

    /// Hide the cursor if the current mode doesn't require one.
    /// Otherwise, display a cursor in the top row, at a correct column.
    ///
    /// # Errors
    ///
    /// may fail if we can't display on the terminal.
    fn draw_cursor(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let offset = self.tab.edit_mode.cursor_offset();
        let index = self.status.menu.input.index();
        canvas.set_cursor(0, offset + index)?;
        canvas.show_cursor(self.tab.edit_mode.show_cursor())?;
        Ok(())
    }

    fn draw_navigate(&self, navigable_mode: Navigate, canvas: &mut dyn Canvas) -> Result<()> {
        match navigable_mode {
            Navigate::CliApplication => self.draw_cli_applications(canvas),
            Navigate::Compress => self.draw_compress(canvas),
            Navigate::Context => self.draw_context(canvas),
            Navigate::EncryptedDrive => self.draw_encrypted_drive(canvas),
            Navigate::History => self.draw_history(canvas),
            Navigate::Marks(mark_action) => self.draw_marks(canvas, mark_action),
            Navigate::RemovableDevices => self.draw_removable(canvas),
            Navigate::TuiApplication => self.draw_shell_menu(canvas),
            Navigate::Shortcut => self.draw_shortcut(canvas, &self.status.menu.shortcut),
            Navigate::Trash => self.draw_trash(canvas),
            Navigate::Cloud => self.draw_cloud(canvas),
            Navigate::Picker => self.draw_picker(canvas),
        }
    }

    /// Display the possible destinations from a selectable content of PathBuf.
    fn draw_shortcut(
        &self,
        canvas: &mut dyn Canvas,
        selectable: &impl Content<PathBuf>,
    ) -> Result<()> {
        let content = selectable.content();
        let (top, bottom) = (self.status.menu.window.top, self.status.menu.window.bottom);
        let len = content.len();
        for (letter, (row, path, attr)) in
            std::iter::zip(('a'..='z').cycle(), enumerated_colored_iter!(content))
                .skip(top)
                .take(min(bottom, len))
        {
            let attr = selectable.attr(row, &attr);
            canvas.print_with_attr(
                row + 1 - top + ContentWindow::WINDOW_MARGIN_TOP,
                2,
                &format!("{letter} "),
                attr,
            )?;
            Self::draw_content_line(
                canvas,
                row + 1 - top,
                path.to_str().context("Unreadable filename")?,
                attr,
            )?;
        }
        Ok(())
    }

    fn draw_history(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let selectable = &self.tab.history;
        let content = selectable.content();
        for (row, pair, attr) in enumerated_colored_iter!(content) {
            let attr = selectable.attr(row, &attr);
            Self::draw_content_line(
                canvas,
                row + 1,
                pair.0.to_str().context("Unreadable filename")?,
                attr,
            )?;
        }
        Ok(())
    }

    fn draw_trash(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let trash = &self.status.menu.trash;
        if trash.content().is_empty() {
            self.draw_trash_is_empty(canvas)
        } else {
            self.draw_trash_content(canvas, trash)
        };
        Ok(())
    }

    fn draw_cloud(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let cloud = &self.status.menu.cloud;
        let mut desc = cloud.desc();
        if let Some((index, metadata)) = &cloud.metadata_repr {
            if index == &cloud.index {
                desc = format!("{desc} - {metadata}");
            }
        }
        let _ = canvas.print_with_attr(
            2,
            2,
            &desc,
            Attr {
                fg: tuikit::attr::Color::LIGHT_BLUE,
                ..Attr::default()
            },
        );
        let content = cloud.content();
        let (top, bottom) = (self.status.menu.window.top, self.status.menu.window.bottom);
        let len = content.len();
        for (row, entry, attr) in enumerated_colored_iter!(content)
            .skip(top)
            .take(min(bottom, len))
        {
            let attr = cloud.attr(row, &attr);
            let _ = canvas.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP + 1 - top,
                4,
                entry.mode_fmt(),
                attr,
            )?;
            let _ = canvas.print_with_attr(
                row + ContentWindow::WINDOW_MARGIN_TOP + 1 - top,
                6,
                entry.path(),
                attr,
            )?;
        }
        Ok(())
    }

    fn draw_trash_content(&self, canvas: &mut dyn Canvas, trash: &Trash) {
        let _ = canvas.print_with_attr(
            1,
            2,
            &self.status.menu.trash.help,
            MENU_COLORS.get().expect("Menu colors should be set").second,
        );
        let content = trash.content();
        let (top, bottom) = (self.status.menu.window.top, self.status.menu.window.bottom);
        let len = content.len();
        for (row, trashinfo, attr) in enumerated_colored_iter!(content)
            .skip(top)
            .take(min(bottom, len))
        {
            let attr = trash.attr(row, &attr);
            let _ = Self::draw_content_line(canvas, row + 1 - top, &trashinfo.to_string(), attr);
        }
    }

    fn draw_picker(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let selectable = &self.status.menu.picker;
        let content = selectable.content();
        if let Some(desc) = &self.status.menu.picker.desc {
            canvas.clear_line(1)?;
            canvas.print_with_attr(
                1,
                2,
                desc,
                MENU_COLORS.get().expect("Menu colors should be set").second,
            )?;
        }
        for (row, pickable, attr) in enumerated_colored_iter!(content) {
            let attr = selectable.attr(row, &attr);
            Self::draw_content_line(canvas, row + 1, pickable, attr)?;
        }
        Ok(())
    }

    fn draw_compress(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let selectable = &self.status.menu.compression;
        let content = selectable.content();
        for (row, compression_method, attr) in enumerated_colored_iter!(content) {
            let attr = selectable.attr(row, &attr);
            Self::draw_content_line(canvas, row + 1, &compression_method.to_string(), attr)?;
        }
        Ok(())
    }

    fn draw_context(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let selectable = &self.status.menu.context;
        canvas.print_with_attr(
            1,
            2,
            "Pick an action.",
            MENU_COLORS.get().expect("Menu colors should be set").second,
        )?;
        let content = selectable.content();
        let space_used = content.len();
        for (letter, (row, desc, attr)) in
            std::iter::zip(('a'..='z').cycle(), enumerated_colored_iter!(content))
        {
            let attr = selectable.attr(row, &attr);
            canvas.print_with_attr(
                row + 1 + ContentWindow::WINDOW_MARGIN_TOP,
                2,
                &format!("{letter} "),
                attr,
            )?;
            Self::draw_content_line(canvas, row + 1, desc, attr)?;
        }
        let more_info = self.tab.context_info(&self.status.internal_settings.opener);
        for (row, text, attr) in enumerated_colored_iter!(more_info) {
            canvas.print_with_attr(
                space_used + row + 1 + ContentWindow::WINDOW_MARGIN_TOP,
                4,
                text,
                attr,
            )?;
        }
        Ok(())
    }

    fn draw_marks(&self, canvas: &mut dyn Canvas, mark_action: MarkAction) -> Result<()> {
        canvas.print(1, 2, mark_action.second_line())?;
        canvas.print_with_attr(
            2,
            4,
            "mark  path",
            MENU_COLORS.get().expect("Menu colors should be set").second,
        )?;

        let content = self.status.menu.marks.as_strings();
        let (top, bottom) = (self.status.menu.window.top, self.status.menu.window.bottom);
        let len = content.len();
        for (row, line, attr) in enumerated_colored_iter!(content)
            .skip(top)
            .take(min(bottom, len))
        {
            let attr = self.status.menu.marks.attr(row, &attr);
            Self::draw_content_line(canvas, row + 1 - top, line, attr)?;
        }
        Ok(())
    }

    // TODO: refactor both methods below with common trait selectable
    fn draw_shell_menu(&self, canvas: &mut dyn Canvas) -> Result<()> {
        canvas.print_with_attr(
            1,
            2,
            self.tab.edit_mode.second_line(),
            MENU_COLORS.get().expect("Menu colors should be set").second,
        )?;

        let content = &self.status.menu.tui_applications.content;
        let (top, bottom) = (self.status.menu.window.top, self.status.menu.window.bottom);
        let len = content.len();
        for (row, command, attr) in enumerated_colored_iter!(content)
            .skip(top)
            .take(min(bottom, len))
        {
            let attr = self.status.menu.tui_applications.attr(row, &attr);
            Self::draw_content_line(canvas, row + 1 - top, command, attr)?;
        }
        Ok(())
    }

    fn draw_cli_applications(&self, canvas: &mut dyn Canvas) -> Result<()> {
        canvas.print_with_attr(
            1,
            2,
            self.tab.edit_mode.second_line(),
            MENU_COLORS.get().expect("Menu colors should be set").second,
        )?;

        let content = &self.status.menu.cli_applications.content;
        let desc_size = self.status.menu.cli_applications.desc_size;
        let (top, bottom) = (self.status.menu.window.top, self.status.menu.window.bottom);
        let len = content.len();
        for (row, cli_command, attr) in enumerated_colored_iter!(content)
            .skip(top)
            .take(min(bottom, len))
        {
            let attr = self.status.menu.cli_applications.attr(row, &attr);
            canvas.print_with_attr(
                row + 1 + ContentWindow::WINDOW_MARGIN_TOP - top,
                4,
                &cli_command.desc,
                attr,
            )?;
            canvas.print_with_attr(
                row + 1 + ContentWindow::WINDOW_MARGIN_TOP - top,
                8 + desc_size,
                &cli_command.executable,
                attr,
            )?;
        }
        Ok(())
    }

    fn draw_encrypted_drive(&self, canvas: &mut dyn Canvas) -> Result<()> {
        self.draw_mountable_devices(&self.status.menu.encrypted_devices, canvas)
    }

    fn draw_removable(&self, canvas: &mut dyn Canvas) -> Result<()> {
        self.draw_mountable_devices(&self.status.menu.removable_devices, canvas)?;
        Ok(())
    }

    fn draw_mountable_devices<T>(
        &self,
        selectable: &impl Content<T>,
        canvas: &mut dyn Canvas,
    ) -> Result<()>
    where
        T: MountRepr,
    {
        canvas.print_with_attr(
            1,
            2,
            ENCRYPTED_DEVICE_BINDS,
            MENU_COLORS.get().expect("Menu colors should be set").second,
        )?;
        let (top, bottom) = (self.status.menu.window.top, self.status.menu.window.bottom);
        let len = selectable.len();
        for (i, device) in selectable
            .content()
            .iter()
            .enumerate()
            .skip(top)
            .take(min(bottom, len))
        {
            self.draw_mountable_device(selectable, i - top, device, canvas)?
        }
        Ok(())
    }

    fn draw_mountable_device<T>(
        &self,
        selectable: &impl Content<T>,
        index: usize,
        device: &T,
        canvas: &mut dyn Canvas,
    ) -> Result<()>
    where
        T: MountRepr,
    {
        let row = calc_line_row(index, &self.tab.window) + 1;
        let attr = selectable.attr(index, &device.attr());
        canvas.print_with_attr(row, 3, &device.device_name()?, attr)?;
        Ok(())
    }

    /// Display a list of edited (deleted, copied, moved, trashed) files for confirmation
    fn draw_confirm(
        &self,
        confirmed_mode: NeedConfirmation,
        canvas: &mut dyn Canvas,
    ) -> Result<()> {
        let dest = path_to_string(&self.tab.directory_of_selected()?);

        Self::draw_content_line(
            canvas,
            0,
            &confirmed_mode.confirmation_string(&dest),
            MENU_COLORS.get().expect("Menu colors should be set").second,
        )?;
        match confirmed_mode {
            NeedConfirmation::EmptyTrash => self.draw_confirm_empty_trash(canvas)?,
            NeedConfirmation::BulkAction => self.draw_confirm_bulk(canvas)?,
            NeedConfirmation::DeleteCloud => self.draw_confirm_delete_cloud(canvas)?,
            _ => self.draw_confirm_default(canvas)?,
        }
        Ok(())
    }

    fn draw_confirm_default(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let content = &self.status.menu.flagged.content;
        for (row, path, attr) in enumerated_colored_iter!(content) {
            Self::draw_content_line(
                canvas,
                row + 2,
                path.to_str().context("Unreadable filename")?,
                attr,
            )?;
        }
        Ok(())
    }

    fn draw_confirm_bulk(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let content = self.status.menu.bulk.format_confirmation();
        for (row, line, attr) in enumerated_colored_iter!(content) {
            Self::draw_content_line(canvas, row + 2, line, attr)?;
        }
        Ok(())
    }

    fn draw_confirm_delete_cloud(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let line = if let Some(selected) = &self.status.menu.cloud.selected() {
            &format!(
                "{desc}{sel}",
                desc = self.status.menu.cloud.desc(),
                sel = selected.path()
            )
        } else {
            "No selected file"
        };
        Self::draw_content_line(canvas, 3, line, Attr::from(Color::LIGHT_RED))?;
        Ok(())
    }

    fn draw_confirm_empty_trash(&self, canvas: &mut dyn Canvas) -> Result<()> {
        if self.status.menu.trash.is_empty() {
            self.draw_trash_is_empty(canvas)
        } else {
            self.draw_confirm_non_empty_trash(canvas)?
        }
        Ok(())
    }

    fn draw_trash_is_empty(&self, canvas: &mut dyn Canvas) {
        let _ = Self::draw_content_line(
            canvas,
            0,
            "Trash is empty",
            MENU_COLORS.get().expect("Menu colors should be set").second,
        );
    }

    fn draw_confirm_non_empty_trash(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let content = self.status.menu.trash.content();
        for (row, trashinfo, attr) in enumerated_colored_iter!(content) {
            let attr = self.status.menu.trash.attr(row, &attr);
            Self::draw_content_line(canvas, row + 4, &trashinfo.to_string(), attr)?;
        }
        Ok(())
    }

    fn draw_content_line(
        canvas: &mut dyn Canvas,
        row: usize,
        text: &str,
        attr: tuikit::attr::Attr,
    ) -> Result<usize> {
        Ok(canvas.print_with_attr(row + ContentWindow::WINDOW_MARGIN_TOP, 4, text, attr)?)
    }
}

impl<'a> Widget for WinSecondary<'a> {}

struct WinSecondaryFirstLine {
    content: Vec<String>,
}

impl Draw for WinSecondaryFirstLine {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        draw_colored_strings(0, 1, &self.content, canvas, false)?;
        Ok(())
    }
}

impl WinSecondaryFirstLine {
    fn new(status: &Status) -> Self {
        Self {
            content: status.current_tab().edit_mode.line_display(status),
        }
    }
}

/// Is responsible for displaying content in the terminal.
/// It uses an already created terminal.
pub struct Display {
    /// The Tuikit terminal attached to the display.
    /// It will print every symbol shown on screen.
    term: Arc<Term>,
}

impl Display {
    /// Returns a new `Display` instance from a `tuikit::term::Term` object.
    pub fn new(term: Arc<Term>) -> Self {
        log_info!("starting display...");
        Self { term }
    }

    /// Display every possible content in the terminal.
    ///
    /// The top line
    ///
    /// The files if we're displaying them
    ///
    /// The cursor if a content is editable
    ///
    /// The help if `Mode::Help`
    ///
    /// The jump_list if `Mode::Jump`
    ///
    /// The completion list if any.
    ///
    /// The preview in preview mode.
    /// Displays one pane or two panes, depending of the width and current
    /// status of the application.
    pub fn display_all(&mut self, status: &MutexGuard<Status>) -> Result<()> {
        self.hide_cursor()?;
        self.term.clear()?;

        let (width, _) = self.term.term_size()?;
        if status.display_settings.dual() && width > MIN_WIDTH_FOR_DUAL_PANE {
            self.draw_dual_pane(status)?
        } else {
            self.draw_single_pane(status)?
        }

        Ok(self.term.present()?)
    }

    /// Hide the curose, clear the terminal and present.
    pub fn force_clear(&mut self) -> Result<()> {
        self.hide_cursor()?;
        self.term.clear()?;
        self.term.present()?;
        Ok(())
    }

    /// Used to force a display of the cursor before leaving the application.
    /// Most of the times we don't need a cursor and it's hidden. We have to
    /// do it unless the shell won't display a cursor anymore.
    pub fn show_cursor(&self) -> Result<()> {
        Ok(self.term.show_cursor(true)?)
    }

    fn hide_cursor(&self) -> Result<()> {
        self.term.set_cursor(0, 0)?;
        Ok(self.term.show_cursor(false)?)
    }

    /// Reads and returns the `tuikit::term::Term` height.
    pub fn height(&self) -> Result<usize> {
        let (_, height) = self.term.term_size()?;
        Ok(height)
    }

    fn size_for_second_window(&self, tab: &Tab) -> Result<usize> {
        if tab.need_second_window() {
            Ok(self.height()? / 2)
        } else {
            Ok(0)
        }
    }

    fn vertical_split<'a>(
        &self,
        win_main: &'a WinMain,
        win_secondary: &'a WinSecondary,
        file_border: Attr,
        menu_border: Attr,
        size: usize,
    ) -> Result<VSplit<'a>> {
        Ok(VSplit::default()
            .split(
                Win::new(win_main)
                    .basis(self.height()? - size)
                    .shrink(4)
                    .border(true)
                    .border_attr(file_border),
            )
            .split(
                Win::new(win_secondary)
                    .basis(size)
                    .shrink(0)
                    .border(true)
                    .border_attr(menu_border),
            ))
    }

    /// Left File, Left Menu, Right File, Right Menu
    fn borders(&self, status: &Status) -> [Attr; 4] {
        let mut borders = [MENU_COLORS
            .get()
            .expect("Menu colors should be set")
            .inert_border; 4];
        let selected_border = MENU_COLORS
            .get()
            .expect("Menu colors should be set")
            .selected_border;
        borders[status.focus.index()] = selected_border;
        borders
    }

    fn draw_dual_pane(&mut self, status: &Status) -> Result<()> {
        let (width, _) = self.term.term_size()?;
        let first_selected = status.focus.is_left();
        let second_selected = !first_selected;
        let attributes_left = WinMainAttributes::new(
            0,
            TabPosition::Left,
            first_selected,
            status.tabs[0].need_second_window(),
        );
        let win_main_left = WinMain::new(status, 0, attributes_left);
        let attributes_right = WinMainAttributes::new(
            width / 2,
            TabPosition::Right,
            second_selected,
            status.tabs[1].need_second_window(),
        );
        let win_main_right = WinMain::new(status, 1, attributes_right);
        let win_second_left = WinSecondary::new(status, 0);
        let win_second_right = WinSecondary::new(status, 1);
        let borders = self.borders(status);
        let percent_left = self.size_for_second_window(&status.tabs[0])?;
        let percent_right = self.size_for_second_window(&status.tabs[1])?;
        let hsplit = HSplit::default()
            .split(self.vertical_split(
                &win_main_left,
                &win_second_left,
                borders[0],
                borders[1],
                percent_left,
            )?)
            .split(self.vertical_split(
                &win_main_right,
                &win_second_right,
                borders[2],
                borders[3],
                percent_right,
            )?);
        Ok(self.term.draw(&hsplit)?)
    }

    fn draw_single_pane(&mut self, status: &Status) -> Result<()> {
        let attributes_left = WinMainAttributes::new(
            0,
            TabPosition::Left,
            true,
            status.tabs[0].need_second_window(),
        );
        let win_main_left = WinMain::new(status, 0, attributes_left);
        let win_second_left = WinSecondary::new(status, 0);
        let percent_left = self.size_for_second_window(&status.tabs[0])?;
        let borders = self.borders(status);
        let win = self.vertical_split(
            &win_main_left,
            &win_second_left,
            borders[0],
            borders[1],
            percent_left,
        )?;
        Ok(self.term.draw(&win)?)
    }
}

fn format_line_nr_hex(line_nr: usize, width: usize) -> String {
    format!("{line_nr:0width$x}")
}

pub const fn color_to_attr(color: Color) -> Attr {
    Attr {
        fg: color,
        bg: Color::Default,
        effect: Effect::empty(),
    }
}

fn draw_colored_strings(
    row: usize,
    offset: usize,
    strings: &[String],
    canvas: &mut dyn Canvas,
    effect_reverse: bool,
) -> Result<()> {
    let mut col = 1;
    for (text, attr) in std::iter::zip(
        strings.iter(),
        MENU_COLORS
            .get()
            .expect("Menu colors should be set")
            .palette()
            .iter()
            .cycle(),
    ) {
        let mut attr = *attr;
        if effect_reverse {
            attr.effect |= Effect::REVERSE;
        }
        col += canvas.print_with_attr(row, offset + col, text, attr)?;
    }
    Ok(())
}

fn draw_clickable_strings(
    row: usize,
    offset: usize,
    elems: &[ClickableString],
    canvas: &mut dyn Canvas,
    effect_reverse: bool,
) -> Result<()> {
    for (elem, attr) in std::iter::zip(
        elems.iter(),
        MENU_COLORS
            .get()
            .expect("Menu colors should be set")
            .palette()
            .iter()
            .cycle(),
    ) {
        let mut attr = *attr;
        if effect_reverse {
            attr.effect |= Effect::REVERSE;
        }
        canvas.print_with_attr(row, offset + elem.col(), elem.text(), attr)?;
    }
    Ok(())
}

fn calc_line_row(i: usize, window: &ContentWindow) -> usize {
    i + ContentWindow::WINDOW_MARGIN_TOP - window.top
}
