use skim::prelude::*;
use tuikit::term::Term;

#[derive()]
pub struct Skimer {
    term: Term,
}

impl Skimer {
    pub fn new(term: Term) -> Self {
        Self { term }
    }

    pub fn no_source(self) -> (Vec<Arc<dyn SkimItem>>, Term) {
        let skim = Skim::new_from_term(self.term);

        (
            skim.run_without(None)
                .map(|out| out.selected_items)
                .unwrap_or_else(|| Vec::new()),
            skim.term,
        )
    }
}
