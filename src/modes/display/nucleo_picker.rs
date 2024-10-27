use std::{
    cmp::min,
    fs::canonicalize,
    path::PathBuf,
    sync::Arc,
    thread::{available_parallelism, spawn},
};

use anyhow::Result;
use nucleo::{pattern, Config, Injector, Nucleo, Utf32String};
use ratatui::{
    prelude::Stylize,
    style::Style,
    text::{Line, Span},
};
use tokio::process::Command as TokioCommand;
use unicode_segmentation::UnicodeSegmentation;
use walkdir::WalkDir;

use crate::io::inject;
use crate::modes::{ContentWindow, Input};

pub enum Direction {
    Up,
    Down,
    PageUp,
    PageDown,
    Index(u16),
}

pub enum FuzzyKind {
    File,
    Line,
    Action,
}

pub struct FuzzyFinder<String: Sync + Send + 'static> {
    /// kind of fuzzy:
    /// Line (match lines into text file),
    /// File (match file against their name),
    /// Action (match an action)
    pub kind: FuzzyKind,
    /// The fuzzy matcher
    pub matcher: Nucleo<String>,
    pub selected: Option<std::string::String>,
    /// matched strings
    pub content: Vec<String>,
    /// typed input by the user
    pub input: Input,
    /// number of parsed item
    pub item_count: usize,
    /// number of matched item
    pub matched_item_count: usize,
    /// selected index. Should always been smaller than matched_item_count
    pub index: usize,
    /// index of the top displayed element in the matcher
    pub top: usize,
    /// height of the terminal window, header & footer included
    pub height: usize,
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
            content: vec![],
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
        self.height = height;
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

    fn index_clamped(&self, matched_item_count: usize) -> usize {
        if matched_item_count == 0 {
            0
        } else {
            min(self.index, matched_item_count.saturating_sub(1))
        }
    }

    /// tick the matcher.
    /// refresh the items if the status changed if force = true.
    pub fn tick(&mut self, force: bool) -> Vec<String> {
        if self.matcher.tick(10).changed || force {
            self.tick_forced()
        } else {
            vec![]
        }
    }

    /// Refresh the content, storing elements around the currently selected.
    fn tick_forced(&mut self) -> Vec<String> {
        let snapshot = self.matcher.snapshot();
        self.item_count = snapshot.item_count() as usize;
        self.matched_item_count = snapshot.matched_item_count() as usize;
        self.index = self.index_clamped(self.matched_item_count);
        let (top, bottom) = self.top_bottom();
        self.top = top;
        let mut indices = vec![];
        let mut matcher = nucleo::Matcher::default();
        snapshot
            .matched_items(top as u32..bottom as u32)
            .enumerate()
            .map(|(index, t)| {
                snapshot.pattern().column_pattern(0).indices(
                    t.matcher_columns[0].slice(..),
                    &mut matcher,
                    &mut indices,
                );
                indices.sort_unstable();
                indices.dedup();
                let highlights = indices.drain(..);
                let text = format_display(&t.matcher_columns[0], highlights);
                if index == self.index {
                    self.selected = Some(text.to_owned());
                }
                text
            })
            .collect()
    }

    /// Calculate the first & last matching index which should be stored in content.
    /// It assumes the index can't change by more than one at a time.
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
    pub fn top_bottom(&self) -> (usize, usize) {
        if self.matched_item_count + ContentWindow::WINDOW_PADDING < self.height {
            // not enough items to fill the display, take everything
            (0, self.matched_item_count)
        } else if self.index < self.top + ContentWindow::WINDOW_PADDING {
            // scroll up by one
            (
                self.top.saturating_sub(1),
                min(
                    self.top + self.height.saturating_sub(ContentWindow::WINDOW_PADDING),
                    self.matched_item_count,
                ),
            )
        } else if self.index + 2 * ContentWindow::WINDOW_PADDING > self.top + self.height {
            // scroll down by one
            (
                self.top + 1,
                min(
                    self.top + self.height.saturating_sub(ContentWindow::WINDOW_PADDING) + 1,
                    self.matched_item_count,
                ),
            )
        } else {
            // don't move
            (
                self.top,
                min(
                    self.top + self.height.saturating_sub(ContentWindow::WINDOW_PADDING),
                    self.matched_item_count,
                ),
            )
        }
    }

    /// Set the new height and refresh the content.
    pub fn resize(&mut self, height: usize) {
        self.height = height;
        self.tick(true);
    }

    /// Returns the selected element, if its index is valid.
    /// It should never return `None` if the content isn't empty.
    pub fn pick(&self) -> Option<std::string::String> {
        // TODO remove logging
        self.log();
        self.selected.to_owned()
    }

    // TODO remove
    fn log(&self) {
        crate::log_info!(
            "index {idx} top {top} offset {off} - matched {mic} - items {itc} - height {hei}",
            idx = self.index,
            top = self.top,
            off = self.index.saturating_sub(self.top),
            mic = self.matched_item_count,
            itc = self.item_count,
            hei = self.height,
        );
    }
}

impl FuzzyFinder<String> {
    fn select_next(&mut self) {
        self.index += 1;
        self.tick(true);
        // TODO remove logging
        self.log()
    }

    fn select_prev(&mut self) {
        self.index = self.index.saturating_sub(1);
        self.tick(true);
        // TODO remove logging
        self.log()
    }

    fn select_clic(&mut self, row: u16) {
        let row = row as usize;
        if row <= ContentWindow::WINDOW_PADDING || row > self.height {
            return;
        }
        self.index = self.top + row - ContentWindow::WINDOW_PADDING - 1;
        // TODO remove logging
        self.log()
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
            if self.index + 1 >= self.content.len() {
                break;
            }
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
        }
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
                let _ = injector.push(line.to_owned(), |line, cols| {
                    cols[0] = line.as_str().into();
                });
            }
        });
    }

    pub fn find_line(&self, tokio_greper: TokioCommand) {
        let injector = self.injector();
        spawn(move || {
            inject(tokio_greper, injector);
        });
    }
}

pub fn parse_line_output(item: &str) -> Result<PathBuf> {
    Ok(canonicalize(PathBuf::from(
        item.split_once(':').unwrap_or(("", "")).0.to_owned(),
    ))?)
}

/// Format a [`Utf32String`] for displaying. Currently:
/// - Delete control characters.
/// - Truncates the string to an appropriate length.
/// - Replaces any newline characters with spaces.
fn format_display(display: &Utf32String, highlights: std::vec::Drain<'_, u32>) -> String {
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

pub fn highlight_line<'a>(
    text: &'a str,
    highlights: std::vec::Drain<'_, u32>,
    is_match: bool,
) -> Line<'a> {
    let highlights_usize: Vec<usize> = highlights.map(|u_32| u_32 as usize).collect();
    let default_style = Style::new().gray().on_black();
    let highlighted_style = Style::new().cyan().on_black();
    create_highlighted_text(
        text,
        &highlights_usize,
        default_style,
        highlighted_style,
        is_match,
    )
}

fn create_highlighted_text<'a>(
    text: &'a str,
    highlighted: &[usize],
    default_style: Style,
    highlighted_style: Style,
    is_match: bool,
) -> Line<'a> {
    let mut spans = vec![];
    let mut current_segment = String::new();
    let mut current_style = default_style;

    let mut highlight_indices = highlighted.iter().copied().peekable();
    let mut next_highlight = highlight_indices.next();

    for (i, grapheme) in text.graphemes(true).enumerate() {
        if Some(i) == next_highlight {
            // Ajouter le segment courant avec le style par défaut
            if !current_segment.is_empty() {
                let mut sp = Span::styled(current_segment.clone(), current_style);
                if is_match {
                    sp = sp.reversed();
                }
                spans.push(sp);
                current_segment.clear();
            }
            // Passer au style "highlighted" pour ce graphème
            current_segment.push_str(grapheme);
            current_style = highlighted_style;
            let mut sp = Span::styled(current_segment.clone(), current_style);
            if is_match {
                sp = sp.reversed();
            }
            spans.push(sp);
            current_segment.clear();
            current_style = default_style;

            // Avancer vers le prochain indice
            next_highlight = highlight_indices.next();
        } else {
            current_segment.push_str(grapheme);
        }
    }

    // Ajouter tout segment restant
    if !current_segment.is_empty() {
        let mut sp = Span::styled(current_segment, current_style);
        if is_match {
            sp = sp.reversed();
        }
        spans.push(sp);
    }

    let ret = Line::from(spans);
    crate::log_info!("texte: {ret:?}");
    ret
}
