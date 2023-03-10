use skim::prelude::*;
use tuikit::term::Term;

use crate::constant_strings_paths::{
    BAT_EXECUTABLE, CAT_EXECUTABLE, GREP_EXECUTABLE, RG_EXECUTABLE,
};
use crate::utils::is_program_in_path;

/// Used to call skim, a clone of fzf.
/// It's a simple wrapper around `Skim` which is used to simplify the interface.
pub struct Skimer {
    skim: Skim,
    previewer: String,
    file_matcher: String,
}

impl Skimer {
    /// Creates a new `Skimer` instance.
    /// `term` is an `Arc<term>` clone of the default term.
    /// It tries to preview with `bat`, but choose `cat` if it can't.
    pub fn new(term: Arc<Term>) -> Self {
        Self {
            skim: Skim::new_from_term(term),
            previewer: pick_first_installed(&[BAT_EXECUTABLE, CAT_EXECUTABLE])
                .expect("Skimer new: at least a previewer should be installed")
                .to_owned(),
            file_matcher: pick_first_installed(&[RG_EXECUTABLE, GREP_EXECUTABLE])
                .expect("Skimer new: at least a line matcher should be installed")
                .to_owned(),
        }
    }

    /// Call skim on its term.
    /// Once the user has selected a file, it will returns its results
    /// as a vec of skimitems.
    /// The preview is enabled by default and we assume the previewer won't be uninstalled during the lifetime
    /// of the application.
    pub fn search_filename(&self, path_str: &str) -> Vec<Arc<dyn SkimItem>> {
        self.skim
            .run_internal(None, path_str.to_owned(), Some(&self.previewer), None)
            .map(|out| out.selected_items)
            .unwrap_or_else(Vec::new)
    }

    pub fn search_line_in_file(&self) -> Vec<Arc<dyn SkimItem>> {
        self.skim
            .run_internal(
                None,
                "".to_owned(),
                None,
                Some(self.file_matcher.to_owned()),
            )
            .map(|out| out.selected_items)
            .unwrap_or_else(Vec::new)
    }
}

fn pick_first_installed<'a>(commands: &'a [&'a str]) -> Option<&'a str> {
    for command in commands {
        let Some(program) = command.split_whitespace().next() else { continue };
        if is_program_in_path(program) {
            return Some(command);
        }
    }
    None
}
