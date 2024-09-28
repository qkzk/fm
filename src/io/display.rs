use std::sync::{Arc, MutexGuard};

use anyhow::{Context, Result};
use tuikit::{
    attr::{Attr, Color},
    prelude::*,
    term::Term,
};

use crate::app::{ClickableLine, ClickableString, Footer, Header, PreviewHeader, Status, Tab};
use crate::common::path_to_string;
use crate::config::{ColorG, Gradient, MENU_ATTRS};
use crate::io::{read_last_log_line, DrawMenu};
use crate::log_info;
use crate::modes::{
    parse_input_mode, BinaryContent, Content, ContentWindow, Display as DisplayMode, Edit,
    FileInfo, HLContent, InputSimple, LineDisplay, MoreInfos, Navigate, NeedConfirmation, Preview,
    SecondLine, Selectable, Text, TextKind, Trash, Tree, TreeLineBuilder, Ueber, Window,
};

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
                    MENU_ATTRS
                        .get()
                        .expect("Menu colors should be set")
                        .first
                        .fg,
                )
                .unwrap_or_default(),
                ColorG::from_tuikit(
                    MENU_ATTRS
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
struct FilesAttributes {
    /// horizontal position, in cells
    x_position: usize,
    /// is this the left or right window ?
    tab_position: TabPosition,
    /// is this tab selected ?
    is_selected: bool,
    /// is there a menuary window ?
    has_window_below: bool,
}

impl FilesAttributes {
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

struct FilesBuilder;

impl FilesBuilder {
    fn dual(status: &Status, width: usize) -> (Files, Files) {
        let first_selected = status.focus.is_left();
        let menu_selected = !first_selected;
        let attributes_left = FilesAttributes::new(
            0,
            TabPosition::Left,
            first_selected,
            status.tabs[0].need_menu_window(),
        );
        let files_left = Files::new(status, 0, attributes_left);
        let attributes_right = FilesAttributes::new(
            width / 2,
            TabPosition::Right,
            menu_selected,
            status.tabs[1].need_menu_window(),
        );
        let files_right = Files::new(status, 1, attributes_right);
        (files_left, files_right)
    }

    fn single(status: &Status) -> Files {
        let attributes_left = FilesAttributes::new(
            0,
            TabPosition::Left,
            true,
            status.tabs[0].need_menu_window(),
        );
        Files::new(status, 0, attributes_left)
    }
}

struct Files<'a> {
    status: &'a Status,
    tab: &'a Tab,
    attributes: FilesAttributes,
}

impl<'a> Draw for Files<'a> {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        if self.should_preview_in_right_tab() {
            self.preview_in_right_tab(canvas)?;
            return Ok(());
        }
        FilesHeader::new(self.status, self.tab, self.attributes.is_selected)?.draw(canvas)?;
        self.copy_progress_bar(canvas)?;
        self.content(canvas)?;
        FilesFooter::new(self.status, self.tab, self.attributes.is_selected)?.draw(canvas)?;
        Ok(())
    }
}

impl<'a> Widget for Files<'a> {}

impl<'a> Files<'a> {
    fn new(status: &'a Status, index: usize, attributes: FilesAttributes) -> Self {
        Self {
            status,
            tab: &status.tabs[index],
            attributes,
        }
    }

    fn should_preview_in_right_tab(&self) -> bool {
        self.status.display_settings.dual()
            && self.is_right()
            && self.status.display_settings.preview()
    }

    fn is_right(&self) -> bool {
        self.attributes.is_right()
    }

    fn content(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        match &self.tab.display_mode {
            DisplayMode::Directory => DirectoryDisplay::new(self).draw(canvas),
            DisplayMode::Tree => TreeDisplay::new(self).draw(canvas),
            DisplayMode::Preview => PreviewDisplay::new(self).draw(canvas),
        }
    }

    /// Display a copy progress bar on the left tab.
    /// Nothing is drawn if there's no copy atm.
    /// If the copy file queue has length > 1, we also display its size.
    fn copy_progress_bar(&self, canvas: &mut dyn Canvas) -> Result<usize> {
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
            MENU_ATTRS
                .get()
                .expect("Menu colors should be set")
                .palette_4,
        )?)
    }

    fn print_flagged_symbol(
        status: &Status,
        canvas: &mut dyn Canvas,
        row: usize,
        path: &std::path::Path,
        attr: &mut Attr,
    ) -> Result<()> {
        if status.menu.flagged.contains(path) {
            attr.effect |= Effect::BOLD;
            canvas.print_with_attr(
                row,
                0,
                "█",
                MENU_ATTRS.get().expect("Menu colors should be set").second,
            )?;
        }
        Ok(())
    }

    fn preview_in_right_tab(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let tab = &self.status.tabs[1];
        let _ = PreviewDisplay::new_with_args(self.status, tab, &self.attributes).draw(canvas);
        let (width, _) = canvas.size()?;
        draw_clickable_strings(
            0,
            0,
            &PreviewHeader::default_preview(self.status, tab, width),
            canvas,
            false,
        )
    }
}

struct DirectoryDisplay<'a> {
    status: &'a Status,
    tab: &'a Tab,
    attributes: &'a FilesAttributes,
}

impl<'a> Draw for DirectoryDisplay<'a> {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        self.files(canvas)?;
        Ok(())
    }
}

impl<'a> DirectoryDisplay<'a> {
    fn new(files: &'a Files) -> Self {
        Self {
            status: files.status,
            tab: files.tab,
            attributes: &files.attributes,
        }
    }

    /// Displays the current directory content, one line per item like in
    /// `ls -l`.
    ///
    /// Only the files around the selected one are displayed.
    /// We reverse the attributes of the selected one, underline the flagged files.
    /// When we display a simpler version, the menu line is used to display the
    /// metadata of the selected file.
    fn files(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let _ = FilesSecondLine::new(self.status, self.tab).draw(canvas);
        self.files_content(canvas)?;
        if !self.attributes.has_window_below && !self.attributes.is_right() {
            let _ = LogLine.draw(canvas);
        }
        Ok(())
    }

    fn files_content(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let (group_size, owner_size) = self.group_owner_size();
        let height = canvas.height()?;
        for (index, file) in self.tab.dir_enum_skip_take() {
            self.files_line(canvas, group_size, owner_size, index, file, height)?;
        }
        Ok(())
    }

    fn group_owner_size(&self) -> (usize, usize) {
        if self.status.display_settings.metadata() {
            (
                self.tab.directory.group_column_width(),
                self.tab.directory.owner_column_width(),
            )
        } else {
            (0, 0)
        }
    }

    fn files_line(
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
        let mut attr = file.attr();
        if index == self.tab.directory.index {
            attr.effect |= Effect::REVERSE;
        }
        let content = Self::format_file_content(self.status, file, owner_size, group_size)?;
        Files::print_flagged_symbol(self.status, canvas, row, &file.path, &mut attr)?;
        let col = if self.status.menu.flagged.contains(&file.path) {
            2
        } else {
            1
        };
        canvas.print_with_attr(row, col, &content, attr)?;
        Ok(())
    }

    fn format_file_content(
        status: &Status,
        file: &FileInfo,
        owner_size: usize,
        group_size: usize,
    ) -> Result<String> {
        if status.display_settings.metadata() {
            file.format(owner_size, group_size)
        } else {
            file.format_simple()
        }
    }
}

struct TreeDisplay<'a> {
    status: &'a Status,
    tab: &'a Tab,
}

impl<'a> Draw for TreeDisplay<'a> {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        self.tree(canvas)?;
        Ok(())
    }
}

impl<'a> TreeDisplay<'a> {
    fn new(files: &'a Files) -> Self {
        Self {
            status: files.status,
            tab: files.tab,
        }
    }

    fn tree(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let _ = FilesSecondLine::new(self.status, self.tab).draw(canvas);
        self.tree_content(canvas)
    }

    fn tree_content(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let left_margin = 1;
        let height = canvas.height()?;

        for (index, line_builder) in self.tab.tree.lines_enum_skip_take(&self.tab.window) {
            Self::tree_line(
                self.status,
                canvas,
                line_builder,
                TreeLinePosition {
                    left_margin,
                    top: self.tab.window.top,
                    index,
                    height,
                },
                self.status.display_settings.metadata(),
            )?;
        }
        Ok(())
    }

    fn tree_line(
        status: &Status,
        canvas: &mut dyn Canvas,
        line_builder: &TreeLineBuilder,
        position_param: TreeLinePosition,
        with_medatadata: bool,
    ) -> Result<()> {
        let (mut col, top, index, height) = position_param.export();
        let row = (index + ContentWindow::WINDOW_MARGIN_TOP).saturating_sub(top);
        if row > height {
            return Ok(());
        }

        let mut attr = line_builder.attr;
        let path = line_builder.path();
        Files::print_flagged_symbol(status, canvas, row, path, &mut attr)?;

        col += Self::tree_metadata(canvas, with_medatadata, row, col, line_builder, attr)?;
        col += if index == 0 { 2 } else { 1 };
        col += canvas.print(row, col, line_builder.prefix())?;
        col += Self::tree_line_calc_flagged_offset(status, path);
        canvas.print_with_attr(row, col, &line_builder.filename(), attr)?;
        Ok(())
    }

    fn tree_metadata(
        canvas: &mut dyn Canvas,
        with_medatadata: bool,
        row: usize,
        col: usize,
        line_builder: &TreeLineBuilder,
        attr: Attr,
    ) -> Result<usize> {
        if with_medatadata {
            Ok(canvas.print_with_attr(row, col, line_builder.metadata(), attr)?)
        } else {
            Ok(0)
        }
    }

    fn tree_line_calc_flagged_offset(status: &Status, path: &std::path::Path) -> usize {
        status.menu.flagged.contains(path) as usize
    }
}

struct PreviewDisplay<'a> {
    status: &'a Status,
    tab: &'a Tab,
    attributes: &'a FilesAttributes,
}

/// Display a scrollable preview of a file.
/// Multiple modes are supported :
/// if the filename extension is recognized, the preview is highlighted,
/// if the file content is recognized as binary, an hex dump is previewed with 16 bytes lines,
/// else the content is supposed to be text and shown as such.
/// It may fail to recognize some usual extensions, notably `.toml`.
/// It may fail to recognize small files (< 1024 bytes).
impl<'a> Draw for PreviewDisplay<'a> {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        let tab = self.tab;
        let window = &tab.window;
        let length = tab.preview.len();
        let line_number_width = length.to_string().len();
        let height = canvas.height()?;
        match &tab.preview {
            Preview::Syntaxed(syntaxed) => {
                self.syntaxed(syntaxed, length, canvas, line_number_width, window)?
            }
            Preview::Binary(bin) => self.binary(bin, length, canvas, window)?,
            Preview::Ueberzug(image) => self.ueberzug(image, canvas)?,
            Preview::Tree(tree_preview) => self.tree_preview(tree_preview, window, canvas)?,
            Preview::Text(colored_text) if matches!(colored_text.kind, TextKind::CliInfo) => {
                self.colored_text(colored_text, length, canvas, window)?
            }
            Preview::Text(text) => {
                impl_preview!(text, tab, length, canvas, line_number_width, window, height)
            }

            Preview::Empty => (),
        }
        Ok(())
    }
}

impl<'a> PreviewDisplay<'a> {
    fn new(files: &'a Files) -> Self {
        Self {
            status: files.status,
            tab: files.tab,
            attributes: &files.attributes,
        }
    }

    fn new_with_args(status: &'a Status, tab: &'a Tab, attributes: &'a FilesAttributes) -> Self {
        Self {
            status,
            tab,
            attributes,
        }
    }

    fn line_number(
        row_position_in_canvas: usize,
        line_number_to_print: usize,
        canvas: &mut dyn Canvas,
    ) -> Result<usize> {
        Ok(canvas.print_with_attr(
            row_position_in_canvas,
            0,
            &line_number_to_print.to_string(),
            MENU_ATTRS.get().expect("Menu colors should be set").first,
        )?)
    }

    fn syntaxed(
        &self,
        syntaxed: &HLContent,
        length: usize,
        canvas: &mut dyn Canvas,
        line_number_width: usize,
        window: &ContentWindow,
    ) -> Result<()> {
        for (i, vec_line) in (*syntaxed).window(window.top, window.bottom, length) {
            let row_position = calc_line_row(i, window);
            Self::line_number(row_position, i + 1, canvas)?;
            for token in vec_line.iter() {
                token.print(canvas, row_position, line_number_width)?;
            }
        }
        Ok(())
    }

    fn binary(
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
                MENU_ATTRS.get().expect("Menu colors should be set").first,
            )?;
            line.print_bytes(canvas, row, line_number_width_hex + 1);
            line.print_ascii(canvas, row, line_number_width_hex + 43);
        }
        Ok(())
    }

    fn ueberzug(&self, image: &Ueber, canvas: &mut dyn Canvas) -> Result<()> {
        let (width, height) = canvas.size()?;
        // image.match_index()?;
        image.draw(
            self.attributes.x_position as u16 + 2,
            3,
            width as u16 - 2,
            height as u16 - 2,
        );
        Ok(())
    }

    fn tree_preview(
        &self,
        tree: &Tree,
        window: &ContentWindow,
        canvas: &mut dyn Canvas,
    ) -> Result<()> {
        let height = canvas.height()?;
        let tree_content = tree.displayable();
        let content = tree_content.lines();
        let length = content.len();

        for (index, tree_line_builder) in
            tree.displayable().window(window.top, window.bottom, length)
        {
            TreeDisplay::tree_line(
                self.status,
                canvas,
                tree_line_builder,
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

    fn colored_text(
        &self,
        colored_text: &Text,
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

struct FilesHeader<'a> {
    status: &'a Status,
    tab: &'a Tab,
    is_selected: bool,
}

impl<'a> Draw for FilesHeader<'a> {
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
            _ => Header::new(self.status, self.tab)?.elems().to_owned(),
        };
        draw_clickable_strings(0, 0, &content, canvas, self.is_selected)?;
        Ok(())
    }
}

impl<'a> FilesHeader<'a> {
    fn new(status: &'a Status, tab: &'a Tab, is_selected: bool) -> Result<Self> {
        Ok(Self {
            status,
            tab,
            is_selected,
        })
    }
}

#[derive(Default)]
struct FilesSecondLine {
    content: Option<String>,
    attr: Option<Attr>,
}

impl Draw for FilesSecondLine {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        match (&self.content, &self.attr) {
            (Some(content), Some(attr)) => canvas.print_with_attr(1, 1, content, *attr)?,
            _ => 0,
        };
        Ok(())
    }
}

impl FilesSecondLine {
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
        let mut attr = file.attr();
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
            MENU_ATTRS.get().expect("Menu colors should be set").second,
        )?;
        Ok(())
    }
}

struct FilesFooter<'a> {
    status: &'a Status,
    tab: &'a Tab,
    is_selected: bool,
}

impl<'a> Draw for FilesFooter<'a> {
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
            _ => Footer::new(self.status, self.tab)?.elems().to_owned(),
        };
        let mut attr = MENU_ATTRS.get().expect("Menu colors should be set").first;
        let last_index = (content.len().saturating_sub(1))
            % MENU_ATTRS
                .get()
                .expect("Menu colors should be set")
                .palette_size();
        let mut background = MENU_ATTRS
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

impl<'a> FilesFooter<'a> {
    fn new(status: &'a Status, tab: &'a Tab, is_selected: bool) -> Result<Self> {
        Ok(Self {
            status,
            tab,
            is_selected,
        })
    }
}

struct Menu<'a> {
    status: &'a Status,
    tab: &'a Tab,
}

impl<'a> Draw for Menu<'a> {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        let mode = self.tab.edit_mode;
        self.cursor(canvas)?;
        MenuFirstLine::new(self.status).draw(canvas)?;
        self.menu_line(canvas)?;
        self.content_per_mode(canvas, mode)?;
        self.binds_per_mode(canvas, mode)?;
        Ok(())
    }
}

impl<'a> Menu<'a> {
    fn new(status: &'a Status, index: usize) -> Self {
        Self {
            status,
            tab: &status.tabs[index],
        }
    }

    /// Hide the cursor if the current mode doesn't require one.
    /// Otherwise, display a cursor in the top row, at a correct column.
    ///
    /// # Errors
    ///
    /// may fail if we can't display on the terminal.
    fn cursor(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let offset = self.tab.edit_mode.cursor_offset();
        let index = self.status.menu.input.index();
        canvas.set_cursor(0, offset + index)?;
        canvas.show_cursor(self.tab.edit_mode.show_cursor())?;
        Ok(())
    }

    fn menu_line(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let menu = MENU_ATTRS.get().expect("Menu colors should be set").second;
        match self.tab.edit_mode {
            Edit::InputSimple(InputSimple::Chmod) => {
                let first = MENU_ATTRS.get().expect("Menu colors should be set").first;
                self.menu_line_chmod(canvas, first, menu)?
            }
            edit => canvas.print_with_attr(1, 2, edit.second_line(), menu)?,
        };
        Ok(())
    }

    fn menu_line_chmod(&self, canvas: &mut dyn Canvas, first: Attr, menu: Attr) -> Result<usize> {
        let mode_parsed = parse_input_mode(&self.status.menu.input.string());
        let mut col = 11;
        for (text, is_valid) in &mode_parsed {
            let attr = if *is_valid { first } else { menu };
            col += 1 + canvas.print_with_attr(1, col, text, attr)?;
        }
        Ok(col)
    }

    fn content_per_mode(&self, canvas: &mut dyn Canvas, mode: Edit) -> Result<()> {
        match mode {
            Edit::Navigate(mode) => self.navigate(mode, canvas),
            Edit::NeedConfirmation(mode) => self.confirm(mode, canvas),
            Edit::InputCompleted(_) => self.completion(canvas),
            Edit::InputSimple(mode) => Self::static_lines(mode.lines(), canvas),
            _ => Ok(()),
        }
    }

    fn binds_per_mode(&self, canvas: &mut dyn Canvas, mode: Edit) -> Result<()> {
        let height = canvas.height()?;
        canvas.clear_line(height - 1)?;
        canvas.print_with_attr(
            height - 1,
            2,
            mode.binds_per_mode(),
            MENU_ATTRS.get().expect("Menu colors should be set").second,
        )?;
        Ok(())
    }

    fn static_lines(lines: &[&str], canvas: &mut dyn Canvas) -> Result<()> {
        for (row, line, attr) in enumerated_colored_iter!(lines) {
            Self::content_line(canvas, row, line, attr)?;
        }
        Ok(())
    }

    fn navigate(&self, navigable_mode: Navigate, canvas: &mut dyn Canvas) -> Result<()> {
        match navigable_mode {
            Navigate::CliApplication => self.cli_applications(canvas),
            Navigate::Cloud => self.cloud(canvas),
            Navigate::Compress => self.compress(canvas),
            Navigate::Context => self.context(canvas),
            Navigate::EncryptedDrive => self.encrypted_drive(canvas),
            Navigate::Flagged => self.flagged(canvas),
            Navigate::History => self.history(canvas),
            Navigate::Marks(_) => self.marks(canvas),
            Navigate::Picker => self.picker(canvas),
            Navigate::RemovableDevices => self.removable_devices(canvas),
            Navigate::Shortcut => self.shortcut(canvas),
            Navigate::Trash => self.trash(canvas),
            Navigate::TuiApplication => self.tui_applications(canvas),
        }
    }

    fn history(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let selectable = &self.tab.history;
        let mut window = ContentWindow::new(selectable.len(), canvas.height()?);
        window.scroll_to(selectable.index);
        selectable.draw_menu(canvas, &window)
    }

    fn trash(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let trash = &self.status.menu.trash;
        if trash.content().is_empty() {
            self.trash_is_empty(canvas)
        } else {
            self.trash_content(canvas, trash)
        };
        Ok(())
    }

    fn trash_content(&self, canvas: &mut dyn Canvas, trash: &Trash) {
        let _ = trash.draw_menu(canvas, &self.status.menu.window);
        let _ = canvas.print_with_attr(
            1,
            2,
            &trash.help,
            MENU_ATTRS.get().expect("Menu colors should be set").second,
        );
    }

    fn trash_is_empty(&self, canvas: &mut dyn Canvas) {
        let _ = Self::content_line(
            canvas,
            0,
            "Trash is empty",
            MENU_ATTRS.get().expect("Menu colors should be set").second,
        );
    }

    fn cloud(&self, canvas: &mut dyn Canvas) -> Result<()> {
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
            MENU_ATTRS
                .get()
                .expect("Menu colors should be set")
                .palette_4,
        );
        cloud.draw_menu(canvas, &self.status.menu.window)
    }

    fn picker(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let selectable = &self.status.menu.picker;
        selectable.draw_menu(canvas, &self.status.menu.window)?;
        if let Some(desc) = &selectable.desc {
            canvas.clear_line(1)?;
            canvas.print_with_attr(
                1,
                2,
                desc,
                MENU_ATTRS.get().expect("Menu colors should be set").second,
            )?;
        }
        Ok(())
    }

    fn context(&self, canvas: &mut dyn Canvas) -> Result<()> {
        self.context_selectable(canvas)?;
        self.context_more_infos(canvas)
    }

    fn context_selectable(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let selectable = &self.status.menu.context;
        canvas.print_with_attr(
            1,
            2,
            "Pick an action.",
            MENU_ATTRS.get().expect("Menu colors should be set").second,
        )?;
        let content = selectable.content();
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
            Self::content_line(canvas, row + 1, desc, attr)?;
        }
        Ok(())
    }

    fn context_more_infos(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let space_used = &self.status.menu.context.content.len();
        let more_info = MoreInfos::new(
            &self.tab.current_file()?,
            &self.status.internal_settings.opener,
        )
        .to_lines();
        for (row, text, attr) in enumerated_colored_iter!(more_info) {
            Self::content_line(canvas, space_used + row + 1, text, attr)?;
        }
        Ok(())
    }

    fn compress(&self, canvas: &mut dyn Canvas) -> Result<()> {
        self.status
            .menu
            .compression
            .draw_menu(canvas, &self.status.menu.window)
    }

    /// Display the possible completion items. The currently selected one is
    /// reversed.
    fn completion(&self, canvas: &mut dyn Canvas) -> Result<()> {
        self.status
            .menu
            .completion
            .draw_menu(canvas, &self.status.menu.window)
    }

    /// Display the possible destinations from a selectable content of PathBuf.
    fn shortcut(&self, canvas: &mut dyn Canvas) -> Result<()> {
        self.status
            .menu
            .shortcut
            .draw_menu(canvas, &self.status.menu.window)
    }
    fn marks(&self, canvas: &mut dyn Canvas) -> Result<()> {
        self.status
            .menu
            .marks
            .draw_menu(canvas, &self.status.menu.window)
    }

    fn tui_applications(&self, canvas: &mut dyn Canvas) -> Result<()> {
        self.status
            .menu
            .tui_applications
            .draw_menu(canvas, &self.status.menu.window)
    }

    fn cli_applications(&self, canvas: &mut dyn Canvas) -> Result<()> {
        self.status
            .menu
            .cli_applications
            .draw_menu(canvas, &self.status.menu.window)
    }

    fn encrypted_drive(&self, canvas: &mut dyn Canvas) -> Result<()> {
        self.status
            .menu
            .encrypted_devices
            .draw_menu(canvas, &self.status.menu.window)
    }

    fn removable_devices(&self, canvas: &mut dyn Canvas) -> Result<()> {
        self.status
            .menu
            .removable_devices
            .draw_menu(canvas, &self.status.menu.window)
    }

    /// Display a list of edited (deleted, copied, moved, trashed) files for confirmation
    fn confirm(&self, confirmed_mode: NeedConfirmation, canvas: &mut dyn Canvas) -> Result<()> {
        let dest = path_to_string(&self.tab.directory_of_selected()?);

        Self::content_line(
            canvas,
            0,
            &confirmed_mode.confirmation_string(&dest),
            MENU_ATTRS.get().expect("Menu colors should be set").second,
        )?;
        match confirmed_mode {
            NeedConfirmation::EmptyTrash => self.confirm_empty_trash(canvas)?,
            NeedConfirmation::BulkAction => self.confirm_bulk(canvas)?,
            NeedConfirmation::DeleteCloud => self.confirm_delete_cloud(canvas)?,
            _ => self.confirm_default(canvas)?,
        }
        Ok(())
    }

    fn confirm_default(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let content = &self.status.menu.flagged.content;
        for (row, path, attr) in enumerated_colored_iter!(content) {
            Self::content_line(
                canvas,
                row + 2,
                path.to_str().context("Unreadable filename")?,
                attr,
            )?;
        }
        Ok(())
    }

    fn confirm_bulk(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let content = self.status.menu.bulk.format_confirmation();
        for (row, line, attr) in enumerated_colored_iter!(content) {
            Self::content_line(canvas, row + 2, line, attr)?;
        }
        Ok(())
    }

    fn confirm_delete_cloud(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let line = if let Some(selected) = &self.status.menu.cloud.selected() {
            &format!(
                "{desc}{sel}",
                desc = self.status.menu.cloud.desc(),
                sel = selected.path()
            )
        } else {
            "No selected file"
        };
        Self::content_line(
            canvas,
            3,
            line,
            MENU_ATTRS
                .get()
                .context("MENU_ATTRS should be set")?
                .palette_4,
        )?;
        Ok(())
    }

    fn confirm_empty_trash(&self, canvas: &mut dyn Canvas) -> Result<()> {
        if self.status.menu.trash.is_empty() {
            self.trash_is_empty(canvas)
        } else {
            self.confirm_non_empty_trash(canvas)?
        }
        Ok(())
    }

    fn confirm_non_empty_trash(&self, canvas: &mut dyn Canvas) -> Result<()> {
        let content = self.status.menu.trash.content();
        for (row, trashinfo, attr) in enumerated_colored_iter!(content) {
            let attr = self.status.menu.trash.attr(row, &attr);
            Self::content_line(canvas, row + 4, &trashinfo.to_string(), attr)?;
        }
        Ok(())
    }

    fn content_line(
        canvas: &mut dyn Canvas,
        row: usize,
        text: &str,
        attr: tuikit::attr::Attr,
    ) -> Result<usize> {
        Ok(canvas.print_with_attr(row + ContentWindow::WINDOW_MARGIN_TOP, 4, text, attr)?)
    }

    fn flagged(&self, canvas: &mut dyn Canvas) -> Result<()> {
        self.flagged_files(canvas)?;
        self.flagged_selected(canvas)
    }

    fn flagged_files(&self, canvas: &mut dyn Canvas) -> Result<()> {
        self.status
            .menu
            .flagged
            .draw_menu(canvas, &self.status.menu.window)
    }

    fn flagged_selected(&self, canvas: &mut dyn Canvas) -> Result<()> {
        if let Some(selected) = self.status.menu.flagged.selected() {
            let fileinfo = FileInfo::new(selected, &self.tab.users)?;
            canvas.print_with_attr(2, 2, &fileinfo.format(6, 6)?, fileinfo.attr())?;
        };
        Ok(())
    }
}

impl<'a> Widget for Menu<'a> {}

struct MenuFirstLine {
    content: Vec<String>,
}

impl Draw for MenuFirstLine {
    fn draw(&self, canvas: &mut dyn Canvas) -> DrawResult<()> {
        draw_colored_strings(0, 1, &self.content, canvas, false)?;
        Ok(())
    }
}

impl MenuFirstLine {
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
            self.dual_pane(status)?
        } else {
            self.single_pane(status)?
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

    fn size_for_menu_window(&self, tab: &Tab) -> Result<usize> {
        if tab.need_menu_window() {
            Ok(self.height()? / 2)
        } else {
            Ok(0)
        }
    }

    fn vertical_split<'a>(
        &self,
        files: &'a Files,
        menu: &'a Menu,
        file_border: Attr,
        menu_border: Attr,
        size: usize,
    ) -> Result<VSplit<'a>> {
        Ok(VSplit::default()
            .split(
                Win::new(files)
                    .basis(self.height()? - size)
                    .shrink(4)
                    .border(true)
                    .border_attr(file_border),
            )
            .split(
                Win::new(menu)
                    .basis(size)
                    .shrink(0)
                    .border(true)
                    .border_attr(menu_border),
            ))
    }

    /// Left File, Left Menu, Right File, Right Menu
    fn borders(&self, status: &Status) -> [Attr; 4] {
        let menu_attrs = MENU_ATTRS.get().expect("MENU_ATTRS should be set");
        let mut borders = [menu_attrs.inert_border; 4];
        let selected_border = menu_attrs.selected_border;
        borders[status.focus.index()] = selected_border;
        borders
    }

    fn dual_pane(&mut self, status: &Status) -> Result<()> {
        let (width, _) = self.term.term_size()?;
        let (files_left, files_right) = FilesBuilder::dual(status, width);
        let menu_left = Menu::new(status, 0);
        let menu_right = Menu::new(status, 1);
        let borders = self.borders(status);
        let percent_left = self.size_for_menu_window(&status.tabs[0])?;
        let percent_right = self.size_for_menu_window(&status.tabs[1])?;
        let hsplit = HSplit::default()
            .split(self.vertical_split(
                &files_left,
                &menu_left,
                borders[0],
                borders[1],
                percent_left,
            )?)
            .split(self.vertical_split(
                &files_right,
                &menu_right,
                borders[2],
                borders[3],
                percent_right,
            )?);
        Ok(self.term.draw(&hsplit)?)
    }

    fn single_pane(&mut self, status: &Status) -> Result<()> {
        let files_left = FilesBuilder::single(status);
        let menu_left = Menu::new(status, 0);
        let percent_left = self.size_for_menu_window(&status.tabs[0])?;
        let borders = self.borders(status);
        let win = self.vertical_split(
            &files_left,
            &menu_left,
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
        MENU_ATTRS
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
        MENU_ATTRS
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
