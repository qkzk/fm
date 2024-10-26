use std::{
    cmp::min,
    fs::canonicalize,
    path::PathBuf,
    sync::Arc,
    thread::{available_parallelism, spawn},
};

use anyhow::Result;
use nucleo::{pattern, Config, Injector, Nucleo, Utf32String};
use tokio::process::Command as TokioCommand;
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
    matcher: Nucleo<String>,
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
    pub fn tick(&mut self, force: bool) {
        if self.matcher.tick(10).changed || force {
            self.tick_forced();
        }
    }

    /// Refresh the content, storing elements around the currently selected.
    fn tick_forced(&mut self) {
        let snapshot = self.matcher.snapshot();
        self.item_count = snapshot.item_count() as usize;
        self.matched_item_count = snapshot.matched_item_count() as usize;
        self.index = self.index_clamped(self.matched_item_count);
        let (top, bottom) = self.top_bottom();
        self.top = top;
        self.content = snapshot
            .matched_items(top as u32..bottom as u32)
            .map(|t| format_display(&t.matcher_columns[0]))
            .collect();
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
    fn top_bottom(&self) -> (usize, usize) {
        if self.matched_item_count + ContentWindow::WINDOW_PADDING < self.height {
            // not enough items to fill the display, take everything
            (0, self.matched_item_count)
        } else if self.index < self.top + ContentWindow::WINDOW_PADDING {
            // scroll up by one
            (
                self.top.saturating_sub(1),
                min(
                    self.top + self.height.saturating_sub(1),
                    self.matched_item_count,
                ),
            )
        } else if self.index + 2 * ContentWindow::WINDOW_PADDING > self.top + self.height {
            // scroll down by one
            (
                self.top + 1,
                min(self.top + self.height + 1, self.matched_item_count),
            )
        } else {
            // don't move
            (
                self.top,
                min(self.top + self.height, self.matched_item_count),
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
    pub fn pick(&self) -> Option<&String> {
        // TODO remove logging
        self.log();
        self.content.get(self.index.saturating_sub(self.top))
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
fn format_display(display: &Utf32String) -> String {
    display
        .slice(..)
        .chars()
        .filter(|ch| !ch.is_control())
        .map(|ch| match ch {
            '\n' => ' ',
            s => s,
        })
        .collect()
}
