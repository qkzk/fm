use skim::prelude::*;
use tuikit::term::Term;

pub struct Skimer {
    skim: Skim,
}

impl Skimer {
    pub fn new(term: Arc<Term>) -> Self {
        Self {
            skim: Skim::new_from_term(term),
        }
    }

    pub fn no_source(&self, path_str: String) -> Vec<Arc<dyn SkimItem>> {
        self.skim
            .run_internal(None, path_str)
            .map(|out| out.selected_items)
            .unwrap_or_else(Vec::new)
    }
}
