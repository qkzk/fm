use std::{
    cmp::{max, min},
    fs::canonicalize,
    path::PathBuf,
    sync::Arc,
    thread::{available_parallelism, spawn},
};

use anyhow::Result;
use nucleo::{pattern, Config, Injector, Nucleo, Utf32String};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use tokio::process::Command as TokioCommand;
use unicode_segmentation::UnicodeSegmentation;
use walkdir::WalkDir;

use crate::modes::{extract_extension, ContentWindow, Icon, Input};
use crate::{
    config::{with_icon, with_icon_metadata},
    io::inject_command,
    modes::FileKind,
};

/// Directions for nucleo picker navigation.
/// Usefull to avoid spreading too much the manipulation
/// in status.
pub enum Direction {
    Up,
    Down,
    PageUp,
    PageDown,
    Start,
    End,
    Index(u16),
}

/// What kind of content is beeing matched ?
/// File: we match against paths,
/// Line & Action we match against strings but actions differ.
pub enum FuzzyKind {
    File,
    Line,
    Action,
}

impl FuzzyKind {
    pub fn is_file(&self) -> bool {
        matches!(self, Self::File)
    }
}

/// The fuzzy picker of file.
/// it may be in one of 3 kinds:
/// - for file, it will match against paths from current folder,
/// - for lines, it will match against any text of a text files from current folder,
/// - for actions, it will match against any text from help, allowing to run an action when you forgot the keybind.
///
/// Internally, it's just :
/// - a [`Nucleo`] matcher,
/// - a few `u32`: index, top (first displayed index), height of the window, item count, matched item count
/// - and the current selection as a string.
///
/// The matcher is used externally by display to get the displayed matches and internally to update
/// the selection when the user type something or move around.
///
/// The interface shouldn't change much, except to add more shortcut.
pub struct FuzzyFinder<String: Sync + Send + 'static> {
    /// kind of fuzzy:
    /// Line (match lines into text file),
    /// File (match file against their name),
    /// Action (match an action)
    pub kind: FuzzyKind,
    /// The fuzzy matcher
    pub matcher: Nucleo<String>,
    /// matched string
    selected: Option<std::string::String>,
    /// typed input by the user
    pub input: Input,
    /// number of parsed item
    pub item_count: u32,
    /// number of matched item
    pub matched_item_count: u32,
    /// selected index. Should always been smaller than matched_item_count
    pub index: u32,
    /// index of the top displayed element in the matcher
    top: u32,
    /// height of the terminal window, header & footer included
    height: u32,
}

impl<String: Sync + Send + 'static> Default for FuzzyFinder<String>
where
    Vec<String>: FromIterator<std::string::String>,
{
    fn default() -> Self {
        let config = Config::DEFAULT.match_paths();
        Self::build(config, FuzzyKind::File)
    }
}

impl<String: Sync + Send + 'static> FuzzyFinder<String>
where
    Vec<String>: FromIterator<std::string::String>,
{
    fn default_thread_count() -> Option<usize> {
        available_parallelism()
            .map(|it| it.get().checked_sub(2).unwrap_or(1))
            .ok()
    }

    fn build_nucleo(config: Config) -> Nucleo<String> {
        Nucleo::new(config, Arc::new(|| {}), Self::default_thread_count(), 1)
    }

    /// Creates a new fuzzy matcher for this kind.
    pub fn new(kind: FuzzyKind) -> Self {
        match kind {
            FuzzyKind::File => Self::default(),
            FuzzyKind::Line => Self::for_lines(),
            FuzzyKind::Action => Self::for_help(),
        }
    }

    fn build(config: Config, kind: FuzzyKind) -> Self {
        Self {
            matcher: Self::build_nucleo(config),
            selected: None,
            item_count: 0,
            matched_item_count: 0,
            index: 0,
            input: Input::default(),
            height: 0,
            top: 0,
            kind,
        }
    }

    fn for_lines() -> Self {
        Self::build(Config::DEFAULT, FuzzyKind::Line)
    }

    fn for_help() -> Self {
        Self::build(Config::DEFAULT, FuzzyKind::Action)
    }

    /// Set the terminal height of the fuzzy picker.
    /// It should always be called after new
    pub fn set_height(mut self, height: usize) -> Self {
        self.height = height as u32;
        self
    }

    /// True iff a preview should be built for this fuzzy finder.
    /// It only makes sense to preview files not lines nor actions.
    pub fn should_preview(&self) -> bool {
        matches!(self.kind, FuzzyKind::File)
    }

    /// Get an [`Injector`] from the internal [`Nucleo`] instance.
    pub fn injector(&self) -> Injector<String> {
        self.matcher.injector()
    }

    /// if insert char: append = true,
    /// if delete char: append = false,
    pub fn update_input(&mut self, append: bool) {
        self.matcher.pattern.reparse(
            0,
            &self.input.string(),
            pattern::CaseMatching::Smart,
            pattern::Normalization::Smart,
            append,
        )
    }

    fn index_clamped(&self, matched_item_count: u32) -> u32 {
        if matched_item_count == 0 {
            0
        } else {
            min(self.index, matched_item_count.saturating_sub(1))
        }
    }

    /// tick the matcher.
    /// refresh the selection if the status changed or if force = true.
    pub fn tick(&mut self, force: bool) {
        if self.matcher.tick(10).changed || force {
            self.tick_forced()
        }
    }

    /// Refresh the content, storing selection, number of items, matched items and updating the top index.
    /// We need to store here the "top" since display can't update fuzzy (it receives a non mutable ref.).
    /// Scrolling is impossible without the update of top once the new index is got.
    fn tick_forced(&mut self) {
        let snapshot = self.matcher.snapshot();
        self.item_count = snapshot.item_count();
        self.matched_item_count = snapshot.matched_item_count();
        self.index = self.index_clamped(self.matched_item_count);
        if let Some(item) = snapshot.get_matched_item(self.index) {
            self.selected = Some(format_display(&item.matcher_columns[0]).to_owned());
        };
        self.update_top();
    }

    fn update_top(&mut self) {
        let (top, _botom) = self.top_bottom();
        self.top = top;
    }

    /// Calculate the first & last matching index which should be stored in content.
    /// It assumes the index can't change by more than one at a time.
    /// Returning both values (top & bottom) allows to avoid mutating self here.
    /// This method can be called in [`crate::io::Display`] to know what matches should be drawn.
    ///
    /// It should only be called after a refresh of the matcher to be sure
    /// the matched_item_count is correct.
    ///
    /// Several cases :
    /// - if there's not enough element to fill the display, take everything.
    /// - if the selection is in the top 4 rows, scroll up if possible.
    /// - if the selection is in the last 4 rows, scroll down if possible.
    /// - otherwise, don't move.
    ///
    /// Scrolling is done only at top or bottom, not in the middle of the screen.
    /// It feels more natural.
    pub fn top_bottom(&self) -> (u32, u32) {
        let used_height = self
            .height
            .saturating_sub(ContentWindow::WINDOW_PADDING_FUZZY);

        let mut top = self.top;
        if self.index <= top {
            // Window is too low
            top = self.index;
        }

        if self.matched_item_count < used_height {
            // not enough items to fill the display, take everything
            (0, self.matched_item_count)
        } else if self.index
            > (top + used_height).saturating_add(ContentWindow::WINDOW_PADDING_FUZZY)
        {
            // window is too high
            let bottom = max(top + used_height, self.matched_item_count);
            (bottom.saturating_sub(used_height) + 1, bottom)
        } else if self.index < top + ContentWindow::WINDOW_PADDING_FUZZY {
            // scroll up by one
            if top + used_height > self.matched_item_count {
                top = self.matched_item_count.saturating_sub(used_height);
            }
            (
                top.saturating_sub(1),
                min(top + used_height, self.matched_item_count),
            )
        } else if self.index + ContentWindow::WINDOW_PADDING_FUZZY > top + used_height {
            // scroll down by one
            (top + 1, min(top + used_height + 1, self.matched_item_count))
        } else {
            // don't move
            (top, min(top + used_height, self.matched_item_count))
        }
    }

    /// Set the new height and refresh the content.
    pub fn resize(&mut self, height: usize) {
        self.height = height as u32;
        self.tick(true);
    }

    /// Returns the selected element, if its index is valid.
    /// It should never return `None` if the content isn't empty.
    pub fn pick(&self) -> Option<std::string::String> {
        #[cfg(debug_assertions)]
        self.log();
        self.selected.to_owned()
    }

    // Do not erase, used for debugging purpose
    #[cfg(debug_assertions)]
    fn log(&self) {
        crate::log_info!(
            "index {idx} top {top} offset {off} - top_bot {top_bot:?} - matched {mic} - items {itc} - height {hei}",
            idx = self.index,
            top = self.top,
            off = self.index.saturating_sub(self.top),
            top_bot = self.top_bottom(),
            mic = self.matched_item_count,
            itc = self.item_count,
            hei = self.height,
        );
    }
}

impl FuzzyFinder<String> {
    fn select_next(&mut self) {
        self.index += 1;
    }

    fn select_prev(&mut self) {
        self.index = self.index.saturating_sub(1);
    }

    fn select_clic(&mut self, row: u16) {
        let row = row as u32;
        if row <= ContentWindow::WINDOW_PADDING_FUZZY || row > self.height {
            return;
        }
        self.index = self.top + row - (ContentWindow::WINDOW_PADDING_FUZZY) - 1;
    }

    fn select_start(&mut self) {
        self.index = 0;
    }

    fn select_end(&mut self) {
        self.index = u32::MAX;
    }

    fn page_up(&mut self) {
        for _ in 0..10 {
            if self.index == 0 {
                break;
            }
            self.select_prev()
        }
    }

    fn page_down(&mut self) {
        for _ in 0..10 {
            self.select_next()
        }
    }

    pub fn navigate(&mut self, direction: Direction) {
        match direction {
            Direction::Up => self.select_prev(),
            Direction::Down => self.select_next(),
            Direction::PageUp => self.page_up(),
            Direction::PageDown => self.page_down(),
            Direction::Index(index) => self.select_clic(index),
            Direction::Start => self.select_start(),
            Direction::End => self.select_end(),
        }
        self.tick(true);
        #[cfg(debug_assertions)]
        self.log();
    }

    pub fn find_files(&self, current_path: PathBuf) {
        let injector = self.injector();
        spawn(move || {
            for entry in WalkDir::new(current_path)
                .into_iter()
                .filter_map(Result::ok)
            {
                let value = entry.path().display().to_string();
                let _ = injector.push(value, |value, cols| {
                    cols[0] = value.as_str().into();
                });
            }
        });
    }

    pub fn find_action(&self, help: String) {
        let injector = self.injector();
        spawn(move || {
            for line in help.lines() {
                injector.push_line(line);
            }
        });
    }

    pub fn find_line(&self, tokio_greper: TokioCommand) {
        let injector = self.injector();
        spawn(move || {
            inject_command(tokio_greper, injector);
        });
    }
}

pub fn parse_line_output(item: &str) -> Result<PathBuf> {
    Ok(canonicalize(PathBuf::from(
        item.split_once(':').unwrap_or(("", "")).0.to_owned(),
    ))?)
}

trait PushLine {
    fn push_line(&self, line: &str);
}

impl PushLine for Injector<String> {
    fn push_line(&self, line: &str) {
        let _ = self.push(line.to_owned(), |line, cols| {
            cols[0] = line.as_str().into();
        });
    }
}

/// Format a [`Utf32String`] for displaying. Currently:
/// - Delete control characters.
/// - Truncates the string to an appropriate length.
/// - Replaces any newline characters with spaces.
fn format_display(display: &Utf32String) -> String {
    display
        .slice(..)
        .chars()
        .filter(|ch| !ch.is_control())
        .map(|ch| match ch {
            '\n' => ' ',
            s => s,
        })
        .collect::<String>()
}

pub fn highlighted_text<'a>(
    text: &'a str,
    highlighted: &[usize],
    is_selected: bool,
    is_file: bool,
) -> Line<'a> {
    let mut spans = create_spans(is_selected);
    if is_file && with_icon() || with_icon_metadata() {
        push_icon(text, is_selected, &mut spans);
    }
    let mut curr_segment = String::new();
    let mut highlight_indices = highlighted.iter().copied().peekable();
    let mut next_highlight = highlight_indices.next();

    for (index, grapheme) in text.graphemes(true).enumerate() {
        if Some(index) == next_highlight {
            if !curr_segment.is_empty() {
                push_clear(&mut spans, &mut curr_segment, is_selected, false);
            }
            curr_segment.push_str(grapheme);
            push_clear(&mut spans, &mut curr_segment, is_selected, true);
            next_highlight = highlight_indices.next();
        } else {
            curr_segment.push_str(grapheme);
        }
    }

    if !curr_segment.is_empty() {
        spans.push(create_span(curr_segment, is_selected, false));
    }

    Line::from(spans)
}

fn push_icon(text: &str, is_selected: bool, spans: &mut Vec<Span>) {
    let file_path = std::path::Path::new(&text);
    let Ok(meta) = file_path.symlink_metadata() else {
        return;
    };
    let file_kind = FileKind::new(&meta, file_path);
    let file_icon = match file_kind {
        FileKind::NormalFile => extract_extension(file_path).icon(),
        file_kind => file_kind.icon(),
    };
    let index = if is_selected { 2 } else { 0 };
    spans.push(Span::styled(file_icon, ARRAY_STYLES[index]))
}

fn push_clear(
    spans: &mut Vec<Span>,
    curr_segment: &mut String,
    is_selected: bool,
    is_highlighted: bool,
) {
    spans.push(create_span(
        curr_segment.clone(),
        is_selected,
        is_highlighted,
    ));
    curr_segment.clear();
}

static DEFAULT_STYLE: Style = Style {
    fg: Some(Color::Gray),
    bg: None,
    add_modifier: Modifier::empty(),
    underline_color: None,
    sub_modifier: Modifier::empty(),
};

static SELECTED: Style = Style {
    fg: Some(Color::Black),
    bg: Some(Color::Cyan),
    add_modifier: Modifier::BOLD,
    underline_color: None,
    sub_modifier: Modifier::empty(),
};

static HIGHLIGHTED: Style = Style {
    fg: Some(Color::White),
    bg: None,
    add_modifier: Modifier::BOLD,
    underline_color: None,
    sub_modifier: Modifier::empty(),
};

static HIGHLIGHTED_SELECTED: Style = Style {
    fg: Some(Color::White),
    bg: Some(Color::Cyan),
    add_modifier: Modifier::BOLD,
    underline_color: None,
    sub_modifier: Modifier::empty(),
};

/// Order is important, item are retrieved by calculating (is_selected)<<1 + (is_highlighted).
static ARRAY_STYLES: [Style; 4] = [DEFAULT_STYLE, HIGHLIGHTED, SELECTED, HIGHLIGHTED_SELECTED];

static SPACER_DEFAULT: &str = "  ";
static SPACER_SELECTED: &str = "> ";

fn create_spans(is_selected: bool) -> Vec<Span<'static>> {
    vec![if is_selected {
        Span::styled(SPACER_SELECTED, SELECTED)
    } else {
        Span::styled(SPACER_DEFAULT, DEFAULT_STYLE)
    }]
}

fn choose_style(is_selected: bool, is_highlighted: bool) -> Style {
    let index = ((is_selected as usize) << 1) + is_highlighted as usize;
    ARRAY_STYLES[index]
}

fn create_span<'a>(curr_segment: String, is_selected: bool, is_highlighted: bool) -> Span<'a> {
    Span::styled(curr_segment, choose_style(is_selected, is_highlighted))
}
