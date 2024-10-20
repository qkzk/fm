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
    use std::{sync::Arc, thread::available_parallelism};

    use anyhow::{anyhow, bail, Context, Result};
    use nucleo::{pattern, Config, Injector, Nucleo, Status, Utf32String};

    use crate::{
        impl_content, impl_selectable,
        modes::{ContentWindow, Input},
    };

    enum NucleoKind {
        DirEntry,
        String,
    }

    pub struct FuzzyFinder<T: Send + Sync + 'static> {
        matcher: Nucleo<T>,
        matcher_state: usize,
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
                matcher_state: 0,
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
            self.update_status(status);
        }

        // TODO update the content, update the index, update the window
        fn update_status(&mut self, status: Status) {
            todo!()
        }

        pub fn pick(&mut self) -> Option<&T> {
            self.matcher
                .snapshot()
                .get_matched_item(self.index as _)
                .map(|item| item.data)
        }
    }
}

pub mod direntry {
    use std::fs::DirEntry;

    use crate::modes::display::nucleo_picker::inner::FuzzyFinder;
    use crate::{impl_content, impl_selectable};

    type Ffd = FuzzyFinder<DirEntry>;
    impl_selectable!(Ffd);
    impl_content!(String, Ffd);
}

pub mod string {
    use crate::modes::display::nucleo_picker::inner::FuzzyFinder;
    use crate::{impl_content, impl_selectable};

    type Ffs = FuzzyFinder<String>;
    impl_selectable!(Ffs);
    impl_content!(String, Ffs);
}
