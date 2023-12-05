use std::cmp::min;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use tuikit::attr::{Attr, Color};
use tuikit::prelude::*;
use tuikit::term::Term;

use crate::app::FirstLine;
use crate::app::Status;
use crate::app::Tab;
use crate::common::path_to_string;
use crate::common::{
    ENCRYPTED_DEVICE_BINDS, HELP_FIRST_SENTENCE, HELP_SECOND_SENTENCE, LOG_FIRST_SENTENCE,
    LOG_SECOND_SENTENCE, TRASH_CONFIRM_LINE,
};
use crate::io::read_last_log_line;
use crate::log_info;
use crate::modes::fileinfo_attr;
use crate::modes::parse_input_mode;
use crate::modes::BinaryContent;
use crate::modes::ColoredText;
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
use crate::modes::SelectableContent;
use crate::modes::TextKind;
use crate::modes::Trash;
use crate::modes::TreePreview;
use crate::modes::Ueberzug;
use crate::modes::Window;
use crate::modes::{calculate_top_bottom, TreeLineMaker};

/// Iter over the content, returning a triplet of `(index, line, attr)`.
macro_rules! enumerated_colored_iter {
    ($t:ident) => {
        std::iter::zip($t.iter().enumerate(), MENU_COLORS.iter().cycle())
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

const FIRST_LINE_COLORS: [Attr; 7] = [
    color_to_attr(Color::Rgb(231, 162, 156)),
    color_to_attr(Color::Rgb(144, 172, 186)),
    color_to_attr(Color::Rgb(214, 125, 83)),
    color_to_attr(Color::Rgb(91, 152, 119)),
    color_to_attr(Color::Rgb(152, 87, 137)),
    color_to_attr(Color::Rgb(230, 189, 87)),
    color_to_attr(Color::Rgb(251, 133, 0)),
];

const MENU_COLORS: [Attr; 10] = [
    color_to_attr(Color::Rgb(236, 250, 250)),
    color_to_attr(Color::Rgb(221, 242, 209)),
    color_to_attr(Color::Rgb(205, 235, 197)),
    color_to_attr(Color::Rgb(190, 227, 186)),
    color_to_attr(Color::Rgb(174, 220, 174)),
    color_to_attr(Color::Rgb(159, 212, 163)),
    color_to_attr(Color::Rgb(174, 220, 174)),
    color_to_attr(Color::Rgb(190, 227, 186)),
    color_to_attr(Color::Rgb(205, 235, 197)),
    color_to_attr(Color::Rgb(221, 242, 209)),
];

const ATTR_YELLOW_BOLD: Attr = Attr {
    fg: Color::YELLOW,
    bg: Color::Default,
    effect: Effect::BOLD,
};

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

trait Height: Canvas {
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
        canvas.clear()?;
        if self.status.display_settings.dual
            && self.is_right()
            && self.status.display_settings.preview
        {
            self.draw_preview_as_second_pane(canvas)?;
            return Ok(());
        }
        self.draw_content(canvas)?;
        WinMainFirstLine::new(self.status, self.tab, self.attributes.is_selected)?.draw(canvas)?;
        Ok(())
    }
}

impl<'a> Widget for WinMain<'a> {}

impl<'a> WinMain<'a> {
    const ATTR_LINE_NR: Attr = color_to_attr(Color::CYAN);

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
        match &self.tab.display_mode {
            DisplayMode::Directory => self.draw_files(canvas),
            DisplayMode::Tree => self.draw_tree(canvas),
            DisplayMode::Preview => self.draw_preview(self.tab, &self.tab.window, canvas),
        }
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
        if !self.attributes.has_window_below {
            let _ = LogLine {}.draw(canvas);
        }
        Ok(None)
    }

    fn draw_files_content(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let len = self.tab.directory.content.len();
        let group_size: usize;
        let owner_size: usize;
        if self.status.display_settings.metadata {
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
            .take(min(len, self.tab.window.bottom))
            .skip(self.tab.window.top)
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
        let content = self.format_file_content(file, owner_size, group_size)?;
        self.print_as_flagged(canvas, row, &file.path, &mut attr)?;
        canvas.print_with_attr(row, 1, &content, attr)?;
        Ok(())
    }

    fn format_file_content(
        &self,
        file: &FileInfo,
        owner_size: usize,
        group_size: usize,
    ) -> Result<String> {
        if self.status.display_settings.metadata {
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
            canvas.print_with_attr(row, 0, "█", ATTR_YELLOW_BOLD)?;
        }
        Ok(())
    }

    fn draw_tree(&self, canvas: &mut dyn Canvas) -> Result<Option<usize>> {
        let _ = WinMainSecondLine::new(self.status, self.tab).draw(canvas);
        let selected_index = self.draw_tree_content(canvas)?;
        Ok(Some(selected_index))
    }

    fn draw_tree_content(&self, canvas: &mut dyn Canvas) -> Result<usize> {
        let left_margin = if self.status.display_settings.metadata {
            0
        } else {
            2
        };
        let height = canvas.height()?;
        let (selected_index, content) = self
            .tab
            .tree
            .content(&self.tab.users, self.status.display_settings.metadata);
        let (top, bottom) = calculate_top_bottom(selected_index, height);
        let length = content.len();

        for (index, wonder) in content
            .iter()
            .enumerate()
            .skip(top)
            .take(min(length, bottom + 0))
        {
            self.draw_tree_maker(
                canvas,
                left_margin,
                top,
                index,
                wonder,
                height,
                self.status.display_settings.metadata,
            )?;
        }
        Ok(selected_index)
    }

    fn draw_tree_maker(
        &self,
        canvas: &mut dyn Canvas,
        left_margin: usize,
        top: usize,
        index: usize,
        tree_line_maker: &TreeLineMaker,
        height: usize,
        display_medatadata: bool,
    ) -> Result<()> {
        let row = index + ContentWindow::WINDOW_MARGIN_TOP - top;
        if row > height {
            return Ok(());
        }

        let s_prefix = tree_line_maker.prefix();
        let mut attr = tree_line_maker.attr();
        let path = tree_line_maker.path();

        self.print_as_flagged(canvas, row, &path, &mut attr)?;

        let col_metadata = if display_medatadata {
            let Some(s_metadata) = tree_line_maker.metadata() else {
                return Err(anyhow!("Metadata should be set."));
            };
            canvas.print_with_attr(row, left_margin, &s_metadata, attr)?
        } else {
            0
        };

        let offset = if index == 0 { 1 } else { 0 };
        let col_tree_prefix = canvas.print(row, left_margin + col_metadata + offset, s_prefix)?;

        canvas.print_with_attr(
            row,
            left_margin + col_metadata + col_tree_prefix + offset,
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
            Self::ATTR_LINE_NR,
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
            Preview::Tree(tree_preview) => self.draw_tree_preview(tree_preview, canvas)?,
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
                Self::ATTR_LINE_NR,
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

    fn draw_tree_preview(&self, tree_preview: &TreePreview, canvas: &mut dyn Canvas) -> Result<()> {
        let height = canvas.height()?;
        let (selected_index, content) = tree_preview.tree.content(&self.tab.users, false);
        let (top, bottom) = calculate_top_bottom(selected_index, height);
        let length = content.len();

        for (index, wonder) in content
            .iter()
            .enumerate()
            .skip(top)
            .take(min(length, bottom + 0))
        {
            self.draw_tree_maker(canvas, 0, top, index, wonder, height, false)?;
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
        draw_colored_strings(
            0,
            0,
            &PreviewFirstLine::make_default_preview(self.status, tab),
            canvas,
            false,
        )?;
        Ok(())
    }
}

struct WinMainFirstLine<'a> {
    status: &'a Status,
    tab: &'a Tab,
    is_selected: bool,
}

impl<'a> Draw for WinMainFirstLine<'a> {
    /// Display the top line on terminal.
    /// Its content depends on the mode.
    /// In normal mode we display the path and number of files.
    /// When a confirmation is needed we ask the user to input `'y'` or
    /// something else.
    /// Returns the result of the number of printed chars.
    /// The colors are reversed when the tab is selected. It gives a visual indication of where he is.
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        let content = match self.tab.display_mode {
            DisplayMode::Preview => PreviewFirstLine::make_preview(self.status, self.tab),
            _ => FirstLine::new(self.status, self.tab)?.strings().to_owned(),
        };
        draw_colored_strings(0, 0, &content, canvas, self.is_selected)?;
        Ok(())
    }
}

impl<'a> WinMainFirstLine<'a> {
    fn new(status: &'a Status, tab: &'a Tab, is_selected: bool) -> Result<Self> {
        Ok(Self {
            status,
            tab,
            is_selected,
        })
    }
}

struct PreviewFirstLine;

impl PreviewFirstLine {
    fn make_preview(status: &Status, tab: &Tab) -> Vec<String> {
        match &tab.preview {
            Preview::Text(text_content) => match text_content.kind {
                TextKind::HELP => Self::make_help(),
                TextKind::LOG => Self::make_log(),
                _ => Self::make_default_preview(status, tab),
            },
            _ => Self::make_default_preview(status, tab),
        }
    }

    fn make_help() -> Vec<String> {
        vec![
            HELP_FIRST_SENTENCE.to_owned(),
            format!(" Version: {v} ", v = std::env!("CARGO_PKG_VERSION")),
            HELP_SECOND_SENTENCE.to_owned(),
        ]
    }

    fn make_log() -> Vec<String> {
        vec![
            LOG_FIRST_SENTENCE.to_owned(),
            LOG_SECOND_SENTENCE.to_owned(),
        ]
    }

    fn _pick_previewed_fileinfo(status: &Status) -> Result<FileInfo> {
        if status.display_settings.dual && status.display_settings.preview {
            status.tabs[0].current_file()
        } else {
            status.current_tab().current_file()
        }
    }

    fn make_default_preview(status: &Status, tab: &Tab) -> Vec<String> {
        if let Ok(fileinfo) = Self::_pick_previewed_fileinfo(status) {
            let mut strings = vec![" Preview ".to_owned()];
            if !tab.preview.is_empty() {
                let index = match &tab.preview {
                    Preview::Ueberzug(image) => image.index + 1,
                    _ => tab.window.bottom,
                };
                strings.push(format!(" {index} / {len} ", len = tab.preview.len()));
            };
            strings.push(format!(" {} ", fileinfo.path.display()));
            strings
        } else {
            vec!["".to_owned()]
        }
    }
}

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
        let (content, attr) = match tab.display_mode {
            DisplayMode::Directory | DisplayMode::Tree => {
                if !status.display_settings.metadata {
                    if let Ok(file) = tab.current_file() {
                        Self::second_line_detailed(&file)
                    } else {
                        (None, None)
                    }
                } else {
                    Self::second_line_simple(status)
                }
            }
            _ => (None, None),
        };
        Self { content, attr }
    }

    fn second_line_detailed(file: &FileInfo) -> (Option<String>, Option<Attr>) {
        let owner_size = file.owner.len();
        let group_size = file.group.len();
        let mut attr = fileinfo_attr(file);
        attr.effect ^= Effect::REVERSE;

        (
            Some(file.format(owner_size, group_size).unwrap_or_default()),
            Some(attr),
        )
    }

    fn second_line_simple(status: &Status) -> (Option<String>, Option<Attr>) {
        (
            Some(status.current_tab().settings.filter.to_string()),
            Some(ATTR_YELLOW_BOLD),
        )
    }
}

struct LogLine;

impl Draw for LogLine {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        let height = canvas.height()?;
        canvas.print_with_attr(height - 1, 4, &read_last_log_line(), ATTR_YELLOW_BOLD)?;
        Ok(())
    }
}

struct WinSecondary<'a> {
    status: &'a Status,
    tab: &'a Tab,
}

impl<'a> Draw for WinSecondary<'a> {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        canvas.clear()?;
        match self.tab.edit_mode {
            Edit::Navigate(mode) => self.draw_navigate(mode, canvas),
            Edit::NeedConfirmation(mode) => self.draw_confirm(mode, canvas),
            Edit::InputCompleted(_) => self.draw_completion(canvas),
            Edit::InputSimple(mode) => Self::draw_static_lines(mode.lines(), canvas),
            _ => return Ok(()),
        }?;
        self.draw_cursor(canvas)?;
        self.draw_second_line(canvas)?;

        WinSecondaryFirstLine::new(self.status).draw(canvas)
    }
}

impl<'a> WinSecondary<'a> {
    const ATTR_YELLOW: Attr = color_to_attr(Color::YELLOW);

    fn new(status: &'a Status, index: usize) -> Self {
        Self {
            status,
            tab: &status.tabs[index],
        }
    }

    fn draw_second_line(&self, canvas: &mut dyn Canvas) -> Result<()> {
        if matches!(self.tab.edit_mode, Edit::InputSimple(InputSimple::Chmod)) {
            let mode_parsed = parse_input_mode(&self.status.menu.input.string());
            let mut col = 11;
            for (text, is_valid) in &mode_parsed {
                let attr = if *is_valid {
                    Attr::from(Color::YELLOW)
                } else {
                    Attr::from(Color::RED)
                };
                col += 1 + canvas.print_with_attr(1, col, text, attr)?;
            }
        }
        Ok(())
    }

    /// Display the possible completion items. The currently selected one is
    /// reversed.
    fn draw_completion(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let content = &self.status.menu.completion.proposals;
        for (row, candidate, attr) in enumerated_colored_iter!(content) {
            let attr = self.status.menu.completion.attr(row, attr);
            Self::draw_content_line(canvas, row, candidate, attr)?;
        }
        Ok(())
    }

    fn draw_static_lines(lines: &[&str], canvas: &mut dyn Canvas) -> Result<()> {
        for (row, line, attr) in enumerated_colored_iter!(lines) {
            Self::draw_content_line(canvas, row, line, *attr)?
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
            Navigate::Bulk => self.draw_bulk(canvas),
            Navigate::CliApplication => self.draw_cli_info(canvas),
            Navigate::Compress => self.draw_compress(canvas),
            Navigate::EncryptedDrive => self.draw_encrypted_drive(canvas),
            Navigate::History => self.draw_history(canvas),
            Navigate::Jump => self.draw_destination(canvas, &self.status.menu.flagged),
            Navigate::Marks(_) => self.draw_marks(canvas),
            Navigate::RemovableDevices => self.draw_removable(canvas),
            Navigate::TuiApplication => self.draw_shell_menu(canvas),
            Navigate::Shortcut => self.draw_destination(canvas, &self.status.menu.shortcut),
            Navigate::Trash => self.draw_trash(canvas),
        }
    }

    /// Display the possible destinations from a selectable content of PathBuf.
    fn draw_destination(
        &self,
        canvas: &mut dyn Canvas,
        selectable: &impl SelectableContent<PathBuf>,
    ) -> Result<()> {
        let content = selectable.content();
        for (row, path, attr) in enumerated_colored_iter!(content) {
            let attr = selectable.attr(row, attr);
            Self::draw_content_line(
                canvas,
                row,
                path.to_str().context("Unreadable filename")?,
                attr,
            )?;
        }
        Ok(())
    }

    fn draw_history(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let selectable = &self.tab.history;
        canvas.print(0, 0, "Go to...")?;
        let content = selectable.content();
        for (row, pair, attr) in enumerated_colored_iter!(content) {
            let attr = selectable.attr(row, attr);
            Self::draw_content_line(
                canvas,
                row,
                pair.0.to_str().context("Unreadable filename")?,
                attr,
            )?;
        }
        Ok(())
    }

    fn draw_bulk(&self, canvas: &mut dyn Canvas) -> Result<()> {
        if let Some(selectable) = &self.status.menu.bulk {
            canvas.print(0, 0, "Action...")?;
            let content = selectable.content();
            for (row, text, attr) in enumerated_colored_iter!(content) {
                let attr = selectable.attr(row, attr);
                Self::draw_content_line(canvas, row, text, attr)?;
            }
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

    fn draw_trash_content(&self, canvas: &mut dyn Canvas, trash: &Trash) {
        let _ = canvas.print(1, 2, TRASH_CONFIRM_LINE);
        let content = trash.content();
        for (row, trashinfo, attr) in enumerated_colored_iter!(content) {
            let attr = trash.attr(row, attr);
            let _ = Self::draw_content_line(canvas, row + 2, &trashinfo.to_string(), attr);
        }
    }

    fn draw_compress(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let selectable = &self.status.menu.compression;
        canvas.print_with_attr(
            2,
            2,
            "Archive and compress the flagged files.",
            Self::ATTR_YELLOW,
        )?;
        canvas.print_with_attr(3, 2, "Pick a compression algorithm.", Self::ATTR_YELLOW)?;
        let content = selectable.content();
        for (row, compression_method, attr) in enumerated_colored_iter!(content) {
            let attr = selectable.attr(row, attr);
            Self::draw_content_line(canvas, row + 3, &compression_method.to_string(), attr)?;
        }
        Ok(())
    }

    fn draw_marks(&self, canvas: &mut dyn Canvas) -> Result<()> {
        canvas.print_with_attr(2, 4, "mark  path", Self::ATTR_YELLOW)?;

        let content = self.status.menu.marks.as_strings();
        for (row, line, attr) in enumerated_colored_iter!(content) {
            let attr = self.status.menu.marks.attr(row, attr);
            Self::draw_content_line(canvas, row, line, attr)?;
        }
        Ok(())
    }

    // TODO: refactor both methods below with common trait selectable
    fn draw_shell_menu(&self, canvas: &mut dyn Canvas) -> Result<()> {
        canvas.print_with_attr(2, 1, "pick a command", Self::ATTR_YELLOW)?;

        let content = &self.status.menu.tui_applications.content;
        for (row, (command, _), attr) in enumerated_colored_iter!(content) {
            let attr = self.status.menu.tui_applications.attr(row, attr);
            Self::draw_content_line(canvas, row + 2, command, attr)?;
        }
        Ok(())
    }

    fn draw_cli_info(&self, canvas: &mut dyn Canvas) -> Result<()> {
        canvas.print_with_attr(2, 1, "pick a command", Self::ATTR_YELLOW)?;

        let content = &self.status.menu.cli_applications.content;
        for (row, cli_command, attr) in enumerated_colored_iter!(content) {
            let attr = self.status.menu.cli_applications.attr(row, attr);
            let col = canvas.print_with_attr(
                row + 2 + ContentWindow::WINDOW_MARGIN_TOP,
                4,
                cli_command.desc,
                attr,
            )?;
            canvas.print_with_attr(
                row + 2 + ContentWindow::WINDOW_MARGIN_TOP,
                8 + col,
                cli_command.executable,
                attr,
            )?;
        }
        Ok(())
    }

    fn draw_encrypted_drive(&self, canvas: &mut dyn Canvas) -> Result<()> {
        self.draw_mountable_devices(&self.status.menu.encrypted_devices, canvas)
    }

    fn draw_removable(&self, canvas: &mut dyn Canvas) -> Result<()> {
        if let Some(removables) = &self.status.menu.removable_devices {
            self.draw_mountable_devices(removables, canvas)?;
        }
        Ok(())
    }

    fn draw_mountable_devices<T>(
        &self,
        selectable: &impl SelectableContent<T>,
        canvas: &mut dyn Canvas,
    ) -> Result<()>
    where
        T: MountRepr,
    {
        canvas.print_with_attr(2, 3, ENCRYPTED_DEVICE_BINDS, Self::ATTR_YELLOW)?;
        for (i, device) in selectable.content().iter().enumerate() {
            self.draw_mountable_device(selectable, i, device, canvas)?
        }
        Ok(())
    }

    fn draw_mountable_device<T>(
        &self,
        selectable: &impl SelectableContent<T>,
        index: usize,
        device: &T,
        canvas: &mut dyn Canvas,
    ) -> Result<()>
    where
        T: MountRepr,
    {
        let row = calc_line_row(index, &self.tab.window) + 2;
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
        log_info!("confirmed action: {:?}", confirmed_mode);
        let dest = path_to_string(&self.tab.directory_of_selected()?);

        Self::draw_content_line(
            canvas,
            0,
            &confirmed_mode.confirmation_string(&dest),
            ATTR_YELLOW_BOLD,
        )?;
        match confirmed_mode {
            NeedConfirmation::EmptyTrash => self.draw_confirm_empty_trash(canvas)?,
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
                *attr,
            )?;
        }
        Ok(())
    }

    fn draw_confirm_empty_trash(&self, canvas: &mut dyn Canvas) -> Result<()> {
        log_info!("draw_confirm_empty_trash");
        if self.status.menu.trash.is_empty() {
            self.draw_trash_is_empty(canvas)
        } else {
            self.draw_confirm_non_empty_trash(canvas)?
        }
        Ok(())
    }

    fn draw_trash_is_empty(&self, canvas: &mut dyn Canvas) {
        let _ = Self::draw_content_line(canvas, 0, "Trash is empty", ATTR_YELLOW_BOLD);
    }

    fn draw_confirm_non_empty_trash(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let content = self.status.menu.trash.content();
        for (row, trashinfo, attr) in enumerated_colored_iter!(content) {
            let attr = self.status.menu.trash.attr(row, attr);
            Self::draw_content_line(canvas, row + 4, &trashinfo.to_string(), attr)?
        }
        Ok(())
    }

    fn draw_content_line(
        canvas: &mut dyn Canvas,
        row: usize,
        text: &str,
        attr: tuikit::attr::Attr,
    ) -> Result<()> {
        canvas.print_with_attr(row + ContentWindow::WINDOW_MARGIN_TOP, 4, text, attr)?;
        Ok(())
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
    const SELECTED_BORDER: Attr = color_to_attr(Color::LIGHT_BLUE);
    const INERT_BORDER: Attr = color_to_attr(Color::Default);

    /// Returns a new `Display` instance from a `tuikit::term::Term` object.
    pub fn new(term: Arc<Term>) -> Self {
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
    pub fn display_all(&mut self, status: &Status) -> Result<()> {
        self.hide_cursor()?;
        self.term.clear()?;

        let (width, _) = self.term.term_size()?;
        if status.display_settings.dual && width > MIN_WIDTH_FOR_DUAL_PANE {
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
        border: Attr,
        size: usize,
    ) -> Result<VSplit<'a>> {
        Ok(VSplit::default()
            .split(
                Win::new(win_main)
                    .basis(self.height()? - size)
                    .shrink(4)
                    .border(true)
                    .border_attr(border),
            )
            .split(
                Win::new(win_secondary)
                    .basis(size)
                    .shrink(0)
                    .border(true)
                    .border_attr(border),
            ))
    }

    fn borders(&self, status: &Status) -> (Attr, Attr) {
        if status.index == 0 {
            (Self::SELECTED_BORDER, Self::INERT_BORDER)
        } else {
            (Self::INERT_BORDER, Self::SELECTED_BORDER)
        }
    }

    fn draw_dual_pane(&mut self, status: &Status) -> Result<()> {
        let (width, _) = self.term.term_size()?;
        let (first_selected, second_selected) = (status.index == 0, status.index == 1);
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
        let (border_left, border_right) = self.borders(status);
        let percent_left = self.size_for_second_window(&status.tabs[0])?;
        let percent_right = self.size_for_second_window(&status.tabs[1])?;
        let hsplit = HSplit::default()
            .split(self.vertical_split(
                &win_main_left,
                &win_second_left,
                border_left,
                percent_left,
            )?)
            .split(self.vertical_split(
                &win_main_right,
                &win_second_right,
                border_right,
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
        let win = self.vertical_split(
            &win_main_left,
            &win_second_left,
            Self::SELECTED_BORDER,
            percent_left,
        )?;
        Ok(self.term.draw(&win)?)
    }
}

fn format_line_nr_hex(line_nr: usize, width: usize) -> String {
    format!("{line_nr:0width$x}")
}

const fn color_to_attr(color: Color) -> Attr {
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
    for (text, attr) in std::iter::zip(strings.iter(), FIRST_LINE_COLORS.iter().cycle()) {
        let mut attr = *attr;
        if effect_reverse {
            attr.effect |= Effect::REVERSE;
        }
        col += canvas.print_with_attr(row, offset + col, text, attr)?;
    }
    Ok(())
}

fn calc_line_row(i: usize, window: &ContentWindow) -> usize {
    i + ContentWindow::WINDOW_MARGIN_TOP - window.top
}
