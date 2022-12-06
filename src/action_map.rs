use strum_macros::{Display, EnumString};

use crate::actioner::Actioner;
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
}

impl ActionMap {
    pub fn match_char(&self, status: &mut Status) -> FmResult<()> {
        let current_tab = status.selected();
        match *self {
            ActionMap::ToggleHidden => Actioner::event_toggle_hidden(current_tab),
            ActionMap::CopyPaste => {
                Actioner::event_copy_paste(current_tab);
                Ok(())
            }
            ActionMap::CutPaste => {
                Actioner::event_cur_paste(current_tab);
                Ok(())
            }
            ActionMap::NewDir => {
                Actioner::event_new_dir(current_tab);
                Ok(())
            }
            ActionMap::NewFile => {
                Actioner::event_new_file(current_tab);
                Ok(())
            }
            ActionMap::Chmod => Actioner::event_chmod(status),
            ActionMap::Exec => {
                Actioner::event_exec(current_tab);
                Ok(())
            }
            ActionMap::Goto => {
                Actioner::event_goto(current_tab);
                Ok(())
            }
            ActionMap::Rename => {
                Actioner::event_rename(current_tab);
                Ok(())
            }
            ActionMap::ClearFlags => Actioner::event_clear_flags(status),
            ActionMap::ToggleFlag => Actioner::event_toggle_flag(status),
            ActionMap::Shell => Actioner::event_shell(current_tab),
            ActionMap::DeleteFile => {
                Actioner::event_delete_file(current_tab);
                Ok(())
            }
            ActionMap::OpenFile => Actioner::event_open_file(current_tab),
            ActionMap::Help => {
                Actioner::event_help(current_tab);
                Ok(())
            }
            ActionMap::Search => {
                Actioner::event_search(current_tab);
                Ok(())
            }
            ActionMap::RegexMatch => {
                Actioner::event_regex_match(current_tab);
                Ok(())
            }
            ActionMap::Quit => {
                Actioner::event_quit(current_tab);
                Ok(())
            }
            ActionMap::FlagAll => Actioner::event_flag_all(status),
            ActionMap::ReverseFlags => Actioner::event_reverse_flags(status),
            ActionMap::Jump => {
                Actioner::event_jump(status);
                Ok(())
            }
            ActionMap::History => {
                Actioner::event_history(current_tab);
                Ok(())
            }
            ActionMap::NvimFilepicker => {
                Actioner::event_nvim_filepicker(current_tab);
                Ok(())
            }
            ActionMap::Sort => {
                Actioner::event_sort(current_tab);
                Ok(())
            }
            ActionMap::Symlink => Actioner::event_symlink(status),
            ActionMap::Preview => Actioner::event_preview(current_tab),
            ActionMap::Shortcut => {
                Actioner::event_shortcut(current_tab);
                Ok(())
            }
            ActionMap::Bulkrename => Actioner::event_bulkrename(status),
            ActionMap::MarksNew => {
                Actioner::event_marks_new(status);
                Ok(())
            }
            ActionMap::MarksJump => {
                Actioner::event_marks_jump(status);
                Ok(())
            }
            ActionMap::Filter => Actioner::event_filter(status.selected()),
            ActionMap::Back => Actioner::event_back(status.selected()),
            ActionMap::Home => Actioner::event_home(status.selected()),
        }
    }
}
