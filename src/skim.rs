use skim::prelude::*;
use tuikit::term::Term;

#[derive(Clone)]
pub struct Skimer {
    term: Arc<Term>,
}

impl Skimer {
    pub fn new(term: Arc<Term>) -> Self {
        Self { term }
    }

    pub fn no_source(&self, path_str: String) -> Vec<Arc<dyn SkimItem>> {
        Skim::new_from_term(self.term.clone())
            .run_internal(None, path_str)
            .map(|out| out.selected_items)
            .unwrap_or_else(Vec::new)
    }
}
