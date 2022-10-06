use std::io::BufRead;

use syntect::easy::HighlightFile;
use syntect::parsing::SyntaxSet;
use syntect::highlighting;

use crate::fileinfo::PathContent;

pub struct Preview {
    pub content: Box<Vec<String>>,
}

impl Default for Preview {
    fn default() -> Self {
        Self {
            content: Box::new(vec![]),
        }
    }
}

impl Preview {
    pub fn fill_preview_lines(&mut self, path_content: &PathContent) {
        self.content = match path_content.selected_file() {
            Some(file) => {
                let reader =
                    std::io::BufReader::new(std::fs::File::open(file.path.clone()).unwrap());
                Box::new(
                    reader
                        .lines()
                        .map(|line| line.unwrap_or_else(|_| "".to_owned()))
                        .collect(),
                )
            }
            None => Box::new(vec![]),
        };
    }

    pub fn empty_preview_lines(&mut self) {
        self.content = Box::new(vec![])
    }

    pub fn bla(&self, path_content: &PathContent) {
        let ss:
        let mut highlighter =
            HighlightFile::new(path_content.selected_file().unwrap().path, &ss, &theme).unwrap();
    }
}
