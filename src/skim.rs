use skim::prelude::*;
use tuikit::term::Term;

#[derive()]
pub struct Skimer {
    term: Arc<Term>,
}

impl Skimer {
    pub fn new(term: Arc<Term>) -> Self {
        Self { term }
    }

    pub fn no_source(self) -> Vec<Arc<dyn SkimItem>> {
        let skim = Skim::new_from_term(self.term);

        skim.run_without(None)
            .map(|out| out.selected_items)
            .unwrap_or_else(|| Vec::new())
    }
}
