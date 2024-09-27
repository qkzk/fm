use crate::{impl_content, impl_selectable};

pub enum PickerCaller {
    Cloud,
    Unknown,
}

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

use crate::io::{DrawMenu, ToPrint};
use crate::modes::Navigate;

impl ToPrint for String {
    fn to_print(&self) -> String {
        self.to_owned()
    }
}

impl DrawMenu<Navigate, String> for Picker {}
