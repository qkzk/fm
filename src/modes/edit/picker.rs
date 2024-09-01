use crate::{impl_content, impl_selectable};

#[derive(Default)]
pub struct Picker {
    pub caller: Option<String>,
    pub index: usize,
    pub content: Vec<String>,
}

impl Picker {
    pub fn clear(&mut self) {
        self.caller = None;
        self.index = 0;
        self.content = vec![];
    }
}

impl_selectable!(Picker);
impl_content!(String, Picker);
