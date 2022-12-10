use strum_macros::{Display, EnumString};

use crate::event_exec::EventExec;
use crate::fm_error::FmResult;
use crate::status::Status;

#[derive(Clone, Debug, Display, EnumString)]
pub enum ActionMap {
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
    Nothing,
}

impl ActionMap {
    pub fn match_char(&self, status: &mut Status) -> FmResult<()> {
        let current_tab = status.selected();
        match *self {
            ActionMap::ToggleHidden => EventExec::event_toggle_hidden(current_tab),
            ActionMap::CopyPaste => {
                EventExec::event_copy_paste(current_tab);
                Ok(())
            }
            ActionMap::CutPaste => {
                EventExec::event_cur_paste(current_tab);
                Ok(())
            }
            ActionMap::NewDir => {
                EventExec::event_new_dir(current_tab);
                Ok(())
            }
            ActionMap::NewFile => {
                EventExec::event_new_file(current_tab);
                Ok(())
            }
            ActionMap::Chmod => EventExec::event_chmod(status),
            ActionMap::Exec => {
                EventExec::event_exec(current_tab);
                Ok(())
            }
            ActionMap::Goto => {
                EventExec::event_goto(current_tab);
                Ok(())
            }
            ActionMap::Rename => {
                EventExec::event_rename(current_tab);
                Ok(())
            }
            ActionMap::ClearFlags => EventExec::event_clear_flags(status),
            ActionMap::ToggleFlag => EventExec::event_toggle_flag(status),
            ActionMap::Shell => EventExec::event_shell(current_tab),
            ActionMap::DeleteFile => {
                EventExec::event_delete_file(current_tab);
                Ok(())
            }
            ActionMap::OpenFile => EventExec::event_open_file(current_tab),
            ActionMap::Help => {
                EventExec::event_help(current_tab);
                Ok(())
            }
            ActionMap::Search => {
                EventExec::event_search(current_tab);
                Ok(())
            }
            ActionMap::RegexMatch => {
                EventExec::event_regex_match(current_tab);
                Ok(())
            }
            ActionMap::Quit => {
                EventExec::event_quit(current_tab);
                Ok(())
            }
            ActionMap::FlagAll => EventExec::event_flag_all(status),
            ActionMap::ReverseFlags => EventExec::event_reverse_flags(status),
            ActionMap::Jump => {
                EventExec::event_jump(status);
                Ok(())
            }
            ActionMap::History => {
                EventExec::event_history(current_tab);
                Ok(())
            }
            ActionMap::NvimFilepicker => {
                EventExec::event_nvim_filepicker(current_tab);
                Ok(())
            }
            ActionMap::Sort => {
                EventExec::event_sort(current_tab);
                Ok(())
            }
            ActionMap::Symlink => EventExec::event_symlink(status),
            ActionMap::Preview => EventExec::event_preview(current_tab),
            ActionMap::Shortcut => {
                EventExec::event_shortcut(current_tab);
                Ok(())
            }
            ActionMap::Bulkrename => EventExec::event_bulkrename(status),
            ActionMap::MarksNew => {
                EventExec::event_marks_new(status);
                Ok(())
            }
            ActionMap::MarksJump => {
                EventExec::event_marks_jump(status);
                Ok(())
            }
            ActionMap::Filter => EventExec::event_filter(status.selected()),
            ActionMap::Back => EventExec::event_back(status.selected()),
            ActionMap::Home => EventExec::event_home(status.selected()),
            ActionMap::Nothing => Ok(()),
        }
    }
}
