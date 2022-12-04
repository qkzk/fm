use crate::fm_error::FmResult;
use crate::status::Status;

#[derive(Clone, Debug)]
pub enum EventChar {
    ToggleHidden,
    CopyPaste,
    CutPaste,
    NewDir,
    NewFile,
    Chmod,
    Exec,
    Goto,
    Rename,
    ClearFlags,
    ToggleFlag,
    Shell,
    DeleteFile,
    OpenFile,
    Help,
    Search,
    RegexMatch,
    Quit,
    FlagAll,
    ReverseFlags,
    Jump,
    History,
    NvimFilepicker,
    Sort,
    Symlink,
    Preview,
    Shortcut,
    Bulkrename,
    MarksNew,
    MarksJump,
    Filter,
    Back,
    Home,
}

impl EventChar {
    pub fn from(s: &str) -> Self {
        match s {
            "ToggleHidden" => EventChar::ToggleHidden,
            "CopyPaste" => EventChar::CopyPaste,
            "CutPaste" => EventChar::CutPaste,
            "NewDir" => EventChar::NewDir,
            "NewFile" => EventChar::NewFile,
            "Chmod" => EventChar::Chmod,
            "Exec" => EventChar::Exec,
            "Goto" => EventChar::Goto,
            "Rename" => EventChar::Rename,
            "ClearFlags" => EventChar::ClearFlags,
            "ToggleFlag" => EventChar::ToggleFlag,
            "Shell" => EventChar::Shell,
            "DeleteFile" => EventChar::DeleteFile,
            "OpenFile" => EventChar::OpenFile,
            "Help" => EventChar::Help,
            "Search" => EventChar::Search,
            "RegexMatch" => EventChar::RegexMatch,
            "Quit" => EventChar::Quit,
            "FlagAll" => EventChar::FlagAll,
            "ReverseFlags" => EventChar::ReverseFlags,
            "Jump" => EventChar::Jump,
            "History" => EventChar::History,
            "NvimFilepicker" => EventChar::NvimFilepicker,
            "Sort" => EventChar::Sort,
            "Symlink" => EventChar::Symlink,
            "Preview" => EventChar::Preview,
            "Shortcut" => EventChar::Shortcut,
            "Bulkrename" => EventChar::Bulkrename,
            "MarksNew" => EventChar::MarksNew,
            "MarksJump" => EventChar::MarksJump,
            "Filter" => EventChar::Filter,
            "Back" => EventChar::Back,
            "Home" => EventChar::Home,
            _ => panic!("Unreadable command"),
        }
    }
    pub fn match_char(&self, status: &mut Status) -> FmResult<()> {
        let current_tab = status.selected();
        match *self {
            EventChar::ToggleHidden => current_tab.event_toggle_hidden(),
            EventChar::CopyPaste => {
                current_tab.event_copy_paste();
                Ok(())
            }
            EventChar::CutPaste => {
                current_tab.event_cur_paste();
                Ok(())
            }
            EventChar::NewDir => {
                current_tab.event_new_dir();
                Ok(())
            }
            EventChar::NewFile => {
                current_tab.event_new_file();
                Ok(())
            }
            EventChar::Chmod => status.event_chmod(),
            EventChar::Exec => {
                current_tab.event_exec();
                Ok(())
            }
            EventChar::Goto => {
                current_tab.event_goto();
                Ok(())
            }
            EventChar::Rename => {
                current_tab.event_rename();
                Ok(())
            }
            EventChar::ClearFlags => status.event_clear_flags(),
            EventChar::ToggleFlag => status.event_toggle_flag(),
            EventChar::Shell => current_tab.event_shell(),
            EventChar::DeleteFile => {
                current_tab.event_delete_file();
                Ok(())
            }
            EventChar::OpenFile => current_tab.event_open_file(),
            EventChar::Help => {
                current_tab.event_help();
                Ok(())
            }
            EventChar::Search => {
                current_tab.event_search();
                Ok(())
            }
            EventChar::RegexMatch => {
                current_tab.event_regex_match();
                Ok(())
            }
            EventChar::Quit => {
                current_tab.event_quit();
                Ok(())
            }
            EventChar::FlagAll => status.event_flag_all(),
            EventChar::ReverseFlags => status.event_reverse_flags(),
            EventChar::Jump => {
                status.event_jump();
                Ok(())
            }
            EventChar::History => {
                current_tab.event_history();
                Ok(())
            }
            EventChar::NvimFilepicker => {
                current_tab.event_nvim_filepicker();
                Ok(())
            }
            EventChar::Sort => {
                current_tab.event_sort();
                Ok(())
            }
            EventChar::Symlink => status.event_symlink(),
            EventChar::Preview => current_tab.event_preview(),
            EventChar::Shortcut => {
                current_tab.event_shortcut();
                Ok(())
            }
            EventChar::Bulkrename => status.event_bulkrename(),
            EventChar::MarksNew => {
                status.event_marks_new();
                Ok(())
            }
            EventChar::MarksJump => {
                status.event_marks_jump();
                Ok(())
            }
            EventChar::Filter => status.selected().event_filter(),
            EventChar::Back => status.selected().event_back(),
            EventChar::Home => status.selected().event_home(),
        }
    }
}

impl Into<String> for EventChar {
    fn into(self) -> String {
        match self {
            EventChar::ToggleHidden => "ToggleHidden".to_owned(),
            EventChar::CopyPaste => "CopyPaste".to_owned(),
            EventChar::CutPaste => "CutPaste".to_owned(),
            EventChar::NewDir => "NewDir".to_owned(),
            EventChar::NewFile => "NewFile".to_owned(),
            EventChar::Chmod => "Chmod".to_owned(),
            EventChar::Exec => "Exec".to_owned(),
            EventChar::Goto => "Goto".to_owned(),
            EventChar::Rename => "Rename".to_owned(),
            EventChar::ClearFlags => "ClearFlags".to_owned(),
            EventChar::ToggleFlag => "ToggleFlag".to_owned(),
            EventChar::Shell => "Shell".to_owned(),
            EventChar::DeleteFile => "DeleteFile".to_owned(),
            EventChar::OpenFile => "OpenFile".to_owned(),
            EventChar::Help => "Help".to_owned(),
            EventChar::Search => "Search".to_owned(),
            EventChar::RegexMatch => "RegexMatch".to_owned(),
            EventChar::Quit => "Quit".to_owned(),
            EventChar::FlagAll => "FlagAll".to_owned(),
            EventChar::ReverseFlags => "ReverseFlags".to_owned(),
            EventChar::Jump => "Jump".to_owned(),
            EventChar::History => "History".to_owned(),
            EventChar::NvimFilepicker => "NvimFilepicker".to_owned(),
            EventChar::Sort => "Sort".to_owned(),
            EventChar::Symlink => "Symlink".to_owned(),
            EventChar::Preview => "Preview".to_owned(),
            EventChar::Shortcut => "Shortcut".to_owned(),
            EventChar::Bulkrename => "Bulkrename".to_owned(),
            EventChar::MarksNew => "MarksNew".to_owned(),
            EventChar::MarksJump => "MarksJump".to_owned(),
            EventChar::Filter => "Filter".to_owned(),
            EventChar::Back => "Back".to_owned(),
            EventChar::Home => "Home".to_owned(),
        }
    }
}
