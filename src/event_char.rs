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
}

impl EventChar {
    pub fn match_char(&self, tabs: &mut Status) -> FmResult<()> {
        let current_status = tabs.selected();
        match *self {
            EventChar::ToggleHidden => current_status.event_toggle_hidden(),
            EventChar::CopyPaste => {
                current_status.event_copy_paste();
                Ok(())
            }
            EventChar::CutPaste => {
                current_status.event_cur_paste();
                Ok(())
            }
            EventChar::NewDir => {
                current_status.event_new_dir();
                Ok(())
            }
            EventChar::NewFile => {
                current_status.event_new_file();
                Ok(())
            }
            EventChar::Chmod => tabs.event_chmod(),
            EventChar::Exec => {
                current_status.event_exec();
                Ok(())
            }
            EventChar::Goto => {
                current_status.event_goto();
                Ok(())
            }
            EventChar::Rename => {
                current_status.event_rename();
                Ok(())
            }
            EventChar::ClearFlags => tabs.event_clear_flags(),
            EventChar::ToggleFlag => tabs.event_toggle_flag(),
            EventChar::Shell => current_status.event_shell(),
            EventChar::DeleteFile => {
                current_status.event_delete_file();
                Ok(())
            }
            EventChar::OpenFile => current_status.event_open_file(),
            EventChar::Help => {
                current_status.event_help();
                Ok(())
            }
            EventChar::Search => {
                current_status.event_search();
                Ok(())
            }
            EventChar::RegexMatch => {
                current_status.event_regex_match();
                Ok(())
            }
            EventChar::Quit => {
                current_status.event_quit();
                Ok(())
            }
            EventChar::FlagAll => tabs.event_flag_all(),
            EventChar::ReverseFlags => tabs.event_reverse_flags(),
            EventChar::Jump => {
                tabs.event_jump();
                Ok(())
            }
            EventChar::History => {
                current_status.event_history();
                Ok(())
            }
            EventChar::NvimFilepicker => {
                current_status.event_nvim_filepicker();
                Ok(())
            }
            EventChar::Sort => {
                current_status.event_sort();
                Ok(())
            }
            EventChar::Symlink => tabs.event_symlink(),
            EventChar::Preview => current_status.event_preview(),
            EventChar::Shortcut => {
                current_status.event_shortcut();
                Ok(())
            }
            EventChar::Bulkrename => tabs.event_bulkrename(),
            EventChar::MarksNew => {
                tabs.event_marks_new();
                Ok(())
            }
            EventChar::MarksJump => {
                tabs.event_marks_jump();
                Ok(())
            }
        }
    }
}
