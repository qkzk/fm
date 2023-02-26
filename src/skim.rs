use skim::prelude::*;
use tuikit::term::Term;

use crate::constant_strings_paths::{BAT_EXECUTABLE, CAT_EXECUTABLE};
use crate::utils::is_program_in_path;

/// Used to call skim, a clone of fzf.
/// It's a simple wrapper around `Skim` which is used to simplify the interface.
pub struct Skimer {
    skim: Skim,
    previewer: String,
}

impl Skimer {
    /// Creates a new `Skimer` instance.
    /// `term` is an `Arc<term>` clone of the default term.
    /// It tries to preview with `bat`, but choose `cat` if it can't.
    pub fn new(term: Arc<Term>) -> Self {
        Self {
            skim: Skim::new_from_term(term),
            previewer: Self::select_previewer().to_owned(),
        }
    }

    fn select_previewer<'a>() -> &'a str {
        let Some(bat) = BAT_EXECUTABLE.split_whitespace().into_iter().next() else { return CAT_EXECUTABLE; };
        if is_program_in_path(bat) {
            BAT_EXECUTABLE
        } else {
            CAT_EXECUTABLE
        }
    }

    /// Call skim on its term.
    /// Once the user has selected a file, it will returns its results
    /// as a vec of skimitems.
    /// The preview is enabled by default and we assume the previewer won't be uninstalled during the lifetime
    /// of the application.
    pub fn no_source(&self, path_str: &str) -> Vec<Arc<dyn SkimItem>> {
        self.skim
            .run_internal(None, path_str.to_owned(), Some(&self.previewer))
            .map(|out| out.selected_items)
            .unwrap_or_else(Vec::new)
    }
}
