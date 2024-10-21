use std::{cmp::min, sync::Arc, thread::available_parallelism};

use nucleo::{pattern, Config, Injector, Nucleo, Utf32String};

use crate::modes::{ContentWindow, Input};

pub struct FuzzyFinder<String: Sync + Send + 'static> {
    matcher: Nucleo<String>,
    pub content: Vec<String>,
    pub item_count: usize,
    pub matched_item_count: usize,
    pub index: usize,
    pub input: Input,
    pub window: ContentWindow,
}

impl<String: Sync + Send + 'static> Default for FuzzyFinder<String>
where
    Vec<String>: FromIterator<std::string::String>,
{
    fn default() -> Self {
        let config = Config::DEFAULT.match_paths();
        Self::new(config)
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

    // TODO send the FMevent here..
    fn build_nucleo(config: Config) -> Nucleo<String> {
        Nucleo::new(config, Arc::new(|| {}), Self::default_thread_count(), 1)
    }

    pub fn new(config: Config) -> Self {
        Self {
            matcher: Self::build_nucleo(config),
            content: vec![],
            item_count: 0,
            matched_item_count: 0,
            index: 0,
            input: Input::default(),
            window: ContentWindow::default(),
        }
    }

    pub fn window(mut self, window: &ContentWindow) -> Self {
        self.window = window.clone();
        self
    }

    /// Get an [`Injector`] from the internal [`Nucleo`] instance.
    pub fn injector(&self) -> Injector<String> {
        self.matcher.injector()
    }

    pub fn update_config(&mut self, config: Config) {
        self.matcher = Self::build_nucleo(config);
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

    fn index_clamped(&self, item_count: usize) -> usize {
        if item_count == 0 {
            0
        } else {
            min(self.index, item_count - 1)
        }
    }

    pub fn tick(&mut self) {
        if self.matcher.tick(10).changed {
            self.tick_forced();
        }
    }

    fn tick_forced(&mut self) {
        let snapshot = self.matcher.snapshot();
        self.item_count = snapshot.item_count() as usize;
        let item_stored = min(
            self.item_count,
            self.window
                .height
                .saturating_sub(ContentWindow::WINDOW_PADDING),
        );
        self.matched_item_count = snapshot.matched_item_count() as usize;
        self.index = self.index_clamped(item_stored);

        // TODO use the range in matched_items to only parse displayed elements here not in display.
        self.content = snapshot
            .matched_items(0..item_stored as u32)
            .map(|t| format_display(&t.matcher_columns[0]))
            .collect();
        crate::log_info!("tick stored {item_stored} items");
        self.window.set_len(item_stored);
        self.window.scroll_to(self.index);
    }

    pub fn resize(&mut self, height: usize) {
        self.window.set_height(height);
        self.tick_forced();
    }

    pub fn pick(&self) -> Option<&String> {
        self.matcher
            .snapshot()
            .get_matched_item(self.index as _)
            .map(|item| item.data)
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
        .collect()
}

use std::cmp::max;

use crate::{impl_content, impl_selectable};

type Ffs = FuzzyFinder<String>;
impl_selectable!(Ffs);
impl_content!(String, Ffs);

impl FuzzyFinder<String> {
    pub fn select_next(&mut self) {
        self.next();
        self.window.scroll_down_one(self.index);
    }

    pub fn select_prev(&mut self) {
        self.prev();
        self.window.scroll_up_one(self.index);
    }

    pub fn select_clic(&mut self, index: usize) {
        self.index = max(index, self.len().saturating_sub(1));
        self.window.scroll_to(self.index)
    }

    pub fn page_down(&mut self) {
        for _ in 0..10 {
            if self.index == 0 {
                break;
            }
            self.select_prev()
        }
    }

    pub fn page_up(&mut self) {
        for _ in 0..10 {
            if self.index >= self.len() {
                break;
            }
            self.select_next()
        }
    }
}
