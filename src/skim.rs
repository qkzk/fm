use skim::prelude::*;
use tuikit::term::Term;

/// Used to call skim, a clone of fzf.
pub struct Skimer {
    skim: Skim,
}

impl Skimer {
    /// Creates a new `Skimer` instance.
    /// `term` is an `Arc<term>` clone of the default term.
    pub fn new(term: Arc<Term>) -> Self {
        Self {
            skim: Skim::new_from_term(term),
        }
    }

    /// Call skim on its term.
    /// Once the user has selected a file, it will returns its results
    /// as a vec of skimitems.
    pub fn no_source(&self, path_str: &str) -> Vec<Arc<dyn SkimItem>> {
        self.skim
            .run_internal(None, path_str.to_owned())
            .map(|out| out.selected_items)
            .unwrap_or_else(Vec::new)
    }
}
