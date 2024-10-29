use crate::io::DrawMenu;
use crate::{impl_content, impl_selectable};

/// Which part of fm asked a picker ?
/// Only cloud uses a picker atm.
pub enum PickerCaller {
    Cloud,
    Unknown,
}

/// A basic picker, allowing to display some text and pick one element.
/// It records a [`PickerCaller`], used to call it back.
#[derive(Default)]
pub struct Picker {
    pub caller: Option<PickerCaller>,
    pub desc: Option<String>,
    pub index: usize,
    pub content: Vec<String>,
}

impl Picker {
    pub fn clear(&mut self) {
        self.caller = None;
        self.index = 0;
        self.content = vec![];
    }

    pub fn set(
        &mut self,
        caller: Option<PickerCaller>,
        desc: Option<String>,
        content: Vec<String>,
    ) {
        self.clear();
        self.caller = caller;
        self.desc = desc;
        self.content = content;
    }
}

impl_selectable!(Picker);
impl_content!(String, Picker);

impl DrawMenu<String> for Picker {}
