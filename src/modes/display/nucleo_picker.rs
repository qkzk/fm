/*

The current situation is really annoying.

It **almost** works.

1. When I use Crossterm in nucleo_picker, no problemo I can display whatever I want.
2. When I use ratatui in nucleo_picker, nothing is displayed properly.

Problem with 1. is that it's not easy to extend the display.
Also, I feel like doing the same thing I already did. Why not use a custom made fuzzy finder and do everything myself ?

Start again with DirENtry
*/
mod inner {
    use std::{cmp::min, sync::Arc, thread::available_parallelism};

    use nucleo::{pattern, Config, Injector, Nucleo, Utf32String};

    use crate::modes::{ContentWindow, Input};

    pub struct FuzzyFinder<T: Send + Sync + 'static> {
        matcher: Nucleo<T>,
        pub content: Vec<String>,
        pub index: usize,
        pub input: Input,
        pub window: ContentWindow,
    }

    impl<T: Send + Sync + 'static> Default for FuzzyFinder<T> {
        fn default() -> Self {
            let config = Config::DEFAULT.match_paths();
            Self::new(config)
        }
    }

    impl<T: Send + Sync + 'static> FuzzyFinder<T> {
        fn default_thread_count() -> Option<usize> {
            available_parallelism()
                .map(|it| it.get().checked_sub(2).unwrap_or(1))
                .ok()
        }

        fn build_nucleo(config: Config) -> Nucleo<T> {
            Nucleo::new(config, Arc::new(|| {}), Self::default_thread_count(), 1)
        }

        pub fn new(config: Config) -> Self {
            Self {
                matcher: Nucleo::new(config, Arc::new(|| {}), Self::default_thread_count(), 1),
                content: vec![],
                index: 0,
                input: Input::default(),
                window: ContentWindow::default(),
            }
        }

        /// Get an [`Injector`] from the internal [`Nucleo`] instance.
        pub fn injector(&self) -> Injector<T> {
            self.matcher.injector()
        }

        pub fn update_config(&mut self, config: Config) {
            self.matcher = Self::build_nucleo(config);
        }

        /// if insert char: append = true,
        /// if delete char: append = false,
        fn update_input(&mut self, append: bool) {
            self.matcher.pattern.reparse(
                0,
                &self.input.string(),
                pattern::CaseMatching::Smart,
                pattern::Normalization::Smart,
                append,
            )
        }

        // TODO call it from somewhere else
        pub fn tick(&mut self) {
            let status = self.matcher.tick(10);
            if status.changed {
                let snapshot = self.matcher.snapshot();

                let item_count = snapshot.item_count() as usize;

                if item_count == 0 {
                    self.index = 0;
                } else {
                    self.index = min(self.index, item_count - 1)
                }
                // TODO use the range in matched_items to only parse displayed elements here not in display.
                self.content = snapshot
                    .matched_items(0..item_count as u32)
                    .map(|t| &t.matcher_columns[0])
                    .map(format_display)
                    .collect();
                self.window.set_len(item_count as _);
                self.window.scroll_to(self.index);
            }
        }

        pub fn pick(&self) -> Option<&T> {
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
}

pub mod direntry {
    use std::cmp::max;
    use walkdir::DirEntry;

    use super::FuzzyFinder;
    use crate::{impl_content, impl_selectable};

    type Ffd = FuzzyFinder<DirEntry>;
    impl_selectable!(Ffd);
    impl_content!(String, Ffd);

    impl FuzzyFinder<DirEntry> {
        pub fn select_next(&mut self) {
            self.next();
            self.window.scroll_to(self.index);
        }

        pub fn select_prev(&mut self) {
            self.prev();
            self.window.scroll_to(self.index);
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
}

pub use inner::FuzzyFinder;

pub mod string {
    use std::cmp::max;

    use super::FuzzyFinder;
    use crate::{impl_content, impl_selectable};

    type Ffs = FuzzyFinder<String>;
    impl_selectable!(Ffs);
    impl_content!(String, Ffs);

    impl FuzzyFinder<String> {
        pub fn select_next(&mut self) {
            self.next();
            self.window.scroll_to(self.index);
        }

        pub fn select_prev(&mut self) {
            self.prev();
            self.window.scroll_to(self.index);
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
}
