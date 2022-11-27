use crate::fm_error::FmResult;
use crate::status::Status;

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
