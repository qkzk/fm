use crate::{impl_content, impl_selectable};

#[derive(Default)]
pub struct Picker {
    pub index: usize,
    pub content: Vec<String>,
}

impl_selectable!(Picker);
impl_content!(String, Picker);
