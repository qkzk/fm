use std::collections::HashMap;

use crate::event_char::EventChar;

#[derive(Clone, Debug)]
pub struct Keybindings {
    pub binds: HashMap<char, EventChar>,
}

impl Default for Keybindings {
    fn default() -> Self {
        Self::new()
    }
}

impl Keybindings {
    pub fn get(&self, key: &char) -> Option<&EventChar> {
        self.binds.get(key)
    }

    pub fn new() -> Self {
        let binds = HashMap::from([
            ('a', EventChar::ToggleHidden),
            ('c', EventChar::CopyPaste),
            ('p', EventChar::CutPaste),
            ('d', EventChar::NewDir),
            ('n', EventChar::NewFile),
            ('m', EventChar::Chmod),
            ('e', EventChar::Exec),
            ('g', EventChar::Goto),
            ('r', EventChar::Rename),
            ('u', EventChar::ClearFlags),
            (' ', EventChar::ToggleFlag),
            ('s', EventChar::Shell),
            ('x', EventChar::DeleteFile),
            ('o', EventChar::OpenFile),
            ('h', EventChar::Help),
            ('/', EventChar::Search),
            ('w', EventChar::RegexMatch),
            ('q', EventChar::Quit),
            ('*', EventChar::FlagAll),
            ('v', EventChar::ReverseFlags),
            ('j', EventChar::Jump),
            ('H', EventChar::History),
            ('i', EventChar::NvimFilepicker),
            ('O', EventChar::Sort),
            ('l', EventChar::Symlink),
            ('P', EventChar::Preview),
            ('G', EventChar::Shortcut),
            ('B', EventChar::Bulkrename),
            ('M', EventChar::MarksNew),
            ('\'', EventChar::MarksJump),
            ('F', EventChar::Filter),
            ('-', EventChar::Back),
            ('~', EventChar::Home),
        ]);
        Self { binds }
    }

    pub fn update_from_config(&mut self, yaml: &serde_yaml::value::Value) {
        if let Some(toggle_hidden) = yaml["toggle_hidden"].as_str().map(|s| s.to_string()) {
            let key = toggle_hidden.chars().next().unwrap_or('a');
            self.binds.insert(key, EventChar::ToggleHidden);
        }
        if let Some(copy_paste) = yaml["copy_paste"].as_str().map(|s| s.to_string()) {
            let key = copy_paste.chars().next().unwrap_or('c');
            self.binds.insert(key, EventChar::CopyPaste);
        }
        if let Some(cut_paste) = yaml["cut_paste"].as_str().map(|s| s.to_string()) {
            let key = cut_paste.chars().next().unwrap_or('p');
            self.binds.insert(key, EventChar::CutPaste);
        }
        if let Some(delete) = yaml["delete"].as_str().map(|s| s.to_string()) {
            let key = delete.chars().next().unwrap_or('x');
            self.binds.insert(key, EventChar::DeleteFile);
        }
        if let Some(chmod) = yaml["chmod"].as_str().map(|s| s.to_string()) {
            let key = chmod.chars().next().unwrap_or('m');
            self.binds.insert(key, EventChar::Chmod);
        }
        if let Some(exec) = yaml["exec"].as_str().map(|s| s.to_string()) {
            let key = exec.chars().next().unwrap_or('e');
            self.binds.insert(key, EventChar::Exec);
        }
        if let Some(newdir) = yaml["newdir"].as_str().map(|s| s.to_string()) {
            let key = newdir.chars().next().unwrap_or('d');
            self.binds.insert(key, EventChar::NewDir);
        }
        if let Some(newfile) = yaml["newfile"].as_str().map(|s| s.to_string()) {
            let key = newfile.chars().next().unwrap_or('n');
            self.binds.insert(key, EventChar::NewFile);
        }
        if let Some(rename) = yaml["rename"].as_str().map(|s| s.to_string()) {
            let key = rename.chars().next().unwrap_or('r');
            self.binds.insert(key, EventChar::Rename);
        }
        if let Some(clear_flags) = yaml["clear_flags"].as_str().map(|s| s.to_string()) {
            let key = clear_flags.chars().next().unwrap_or('u');
            self.binds.insert(key, EventChar::ClearFlags);
        }
        if let Some(toggle_flag) = yaml["toggle_flag"].as_str().map(|s| s.to_string()) {
            let key = toggle_flag.chars().next().unwrap_or(' ');
            self.binds.insert(key, EventChar::ToggleFlag);
        }
        if let Some(shell) = yaml["shell"].as_str().map(|s| s.to_string()) {
            let key = shell.chars().next().unwrap_or('s');
            self.binds.insert(key, EventChar::Shell);
        }
        if let Some(open_file) = yaml["open_file"].as_str().map(|s| s.to_string()) {
            let key = open_file.chars().next().unwrap_or('o');
            self.binds.insert(key, EventChar::OpenFile);
        }
        if let Some(help) = yaml["help"].as_str().map(|s| s.to_string()) {
            let key = help.chars().next().unwrap_or('h');
            self.binds.insert(key, EventChar::Help);
        }
        if let Some(search) = yaml["search"].as_str().map(|s| s.to_string()) {
            let key = search.chars().next().unwrap_or('/');
            self.binds.insert(key, EventChar::Search);
        }
        if let Some(quit) = yaml["quit"].as_str().map(|s| s.to_string()) {
            let key = quit.chars().next().unwrap_or('q');
            self.binds.insert(key, EventChar::Quit);
        }
        if let Some(goto) = yaml["goto"].as_str().map(|s| s.to_string()) {
            let key = goto.chars().next().unwrap_or('g');
            self.binds.insert(key, EventChar::Goto);
        }
        if let Some(flag_all) = yaml["flag_all"].as_str().map(|s| s.to_string()) {
            let key = flag_all.chars().next().unwrap_or('*');
            self.binds.insert(key, EventChar::FlagAll);
        }
        if let Some(reverse_flags) = yaml["reverse_flags"].as_str().map(|s| s.to_string()) {
            let key = reverse_flags.chars().next().unwrap_or('v');
            self.binds.insert(key, EventChar::ReverseFlags);
        }
        if let Some(regex_match) = yaml["regex_match"].as_str().map(|s| s.to_string()) {
            let key = regex_match.chars().next().unwrap_or('w');
            self.binds.insert(key, EventChar::RegexMatch);
        }
        if let Some(jump) = yaml["jump"].as_str().map(|s| s.to_string()) {
            let key = jump.chars().next().unwrap_or('j');
            self.binds.insert(key, EventChar::Jump);
        }
        if let Some(nvim) = yaml["nvim"].as_str().map(|s| s.to_string()) {
            let key = nvim.chars().next().unwrap_or('i');
            self.binds.insert(key, EventChar::NvimFilepicker);
        }
        if let Some(sort_by) = yaml["sort_by"].as_str().map(|s| s.to_string()) {
            let key = sort_by.chars().next().unwrap_or('O');
            self.binds.insert(key, EventChar::Sort);
        }
        if let Some(symlink) = yaml["symlink"].as_str().map(|s| s.to_string()) {
            let key = symlink.chars().next().unwrap_or('S');
            self.binds.insert(key, EventChar::Symlink);
        }
        if let Some(preview) = yaml["preview"].as_str().map(|s| s.to_string()) {
            let key = preview.chars().next().unwrap_or('P');
            self.binds.insert(key, EventChar::Preview);
        }
        if let Some(history) = yaml["history"].as_str().map(|s| s.to_string()) {
            let key = history.chars().next().unwrap_or('H');
            self.binds.insert(key, EventChar::History);
        }
        if let Some(shortcut) = yaml["shortcut"].as_str().map(|s| s.to_string()) {
            let key = shortcut.chars().next().unwrap_or('G');
            self.binds.insert(key, EventChar::Shortcut);
        }
        if let Some(bulkrename) = yaml["bulkrename"].as_str().map(|s| s.to_string()) {
            let key = bulkrename.chars().next().unwrap_or('B');
            self.binds.insert(key, EventChar::Bulkrename);
        }
        if let Some(marks_new) = yaml["marks_new"].as_str().map(|s| s.to_string()) {
            let key = marks_new.chars().next().unwrap_or('M');
            self.binds.insert(key, EventChar::MarksNew);
        }
        if let Some(marks_jump) = yaml["marks_jump"].as_str().map(|s| s.to_string()) {
            let key = marks_jump.chars().next().unwrap_or('\'');
            self.binds.insert(key, EventChar::MarksJump);
        }
        if let Some(filter) = yaml["filter"].as_str().map(|s| s.to_string()) {
            let key = filter.chars().next().unwrap_or('f');
            self.binds.insert(key, EventChar::Filter);
        }
        if let Some(back) = yaml["back"].as_str().map(|s| s.to_string()) {
            let key = back.chars().next().unwrap_or('-');
            self.binds.insert(key, EventChar::Back);
        }
        if let Some(home) = yaml["home"].as_str().map(|s| s.to_string()) {
            let key = home.chars().next().unwrap_or('~');
            self.binds.insert(key, EventChar::Home);
        }
    }

    pub fn update(&mut self, yaml: &serde_yaml::value::Value) {
        for i in 32_u8..=127_u8 {
            let c = i as char;
            let s = c.to_string();
            if let Some(v) = yaml[s].as_str().map(|s| s.to_string()) {
                self.binds.insert(c, EventChar::from(&v));
            }
        }
    }

    pub fn to_hashmap(&self) -> HashMap<String, String> {
        let mut reverse = HashMap::new();
        for (k, v) in self.binds.clone().into_iter() {
            let _ = reverse.insert(v.into(), k.into());
        }
        reverse
    }
}
