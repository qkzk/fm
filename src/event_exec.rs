use crate::completion::CompletionKind;
use crate::constant_strings_paths::NVIM_RPC_SENDER;
use crate::mode::{ConfirmedAction, InputKind, MarkAction, Mode};
/// It mutates `Status` or its children `tab`.
        status.selected().mode = Mode::InputSimple(InputKind::Chmod);
        status.selected().mode = Mode::InputSimple(InputKind::Marks(MarkAction::New));
        status.selected().mode = Mode::InputSimple(InputKind::Marks(MarkAction::Jump));
    fn _exec_confirmed_action(
        status: &mut Status,
        confirmed_action: ConfirmedAction,
    ) -> FmResult<()> {
        match confirmed_action {
            ConfirmedAction::Delete => Self::exec_delete_files(status),
            ConfirmedAction::Move => Self::exec_cut_paste(status),
            ConfirmedAction::Copy => Self::exec_copy_paste(status),
    pub fn exec_confirmed_action(
        status: &mut Status,
        confirmed_action: ConfirmedAction,
    ) -> FmResult<()> {
        Self::_exec_confirmed_action(status, confirmed_action)?;
            Mode::Preview => tab.line_index = tab.window.top,
            Mode::Preview => tab.line_index = tab.window.bottom,
        tab.mode = Mode::NeedConfirmation(ConfirmedAction::Copy);
        tab.mode = Mode::NeedConfirmation(ConfirmedAction::Move);
        tab.mode = Mode::InputSimple(InputKind::Newdir);
        tab.mode = Mode::InputSimple(InputKind::Newfile);
        tab.mode = Mode::InputCompleted(CompletionKind::Exec);
        if let Some(file_info) = tab.path_content.selected_file() {
            if let FileKind::NormalFile = file_info.file_kind {
                tab.preview = Preview::new(file_info)?;
        tab.mode = Mode::NeedConfirmation(ConfirmedAction::Delete);
        tab.mode = Mode::Preview;
        tab.mode = Mode::InputCompleted(CompletionKind::Search);
        tab.mode = Mode::InputSimple(InputKind::RegexMatch);
        tab.mode = Mode::InputSimple(InputKind::Sort);
        tab.mode = Mode::InputSimple(InputKind::Rename);
        tab.mode = Mode::InputCompleted(CompletionKind::Goto);
                    NVIM_RPC_SENDER,
        tab.mode = Mode::InputSimple(InputKind::Filter);
            Mode::Normal | Mode::Preview => EventExec::event_up_one_row(status.selected()),
            Mode::InputCompleted(_) => {
            Mode::Normal | Mode::Preview => EventExec::event_down_one_row(status.selected()),
            Mode::InputCompleted(_) => status.selected().completion.next(),
            Mode::InputSimple(_) | Mode::InputCompleted(_) => {
            Mode::InputSimple(_) | Mode::InputCompleted(_) => {
            Mode::InputSimple(_) | Mode::InputCompleted(_) => {
            Mode::InputSimple(_) | Mode::InputCompleted(_) => {
            Mode::Normal | Mode::Preview => EventExec::event_go_top(status.selected()),
            Mode::Normal | Mode::Preview => EventExec::event_go_bottom(status.selected()),
            Mode::Normal | Mode::Preview => EventExec::event_page_up(status.selected()),
            Mode::Normal | Mode::Preview => EventExec::event_page_down(status.selected()),
            Mode::InputSimple(InputKind::Rename) => EventExec::exec_rename(status.selected())?,
            Mode::InputSimple(InputKind::Newfile) => EventExec::exec_newfile(status.selected())?,
            Mode::InputSimple(InputKind::Newdir) => EventExec::exec_newdir(status.selected())?,
            Mode::InputSimple(InputKind::Chmod) => EventExec::exec_chmod(status)?,
            Mode::InputSimple(InputKind::RegexMatch) => EventExec::exec_regex(status)?,
            Mode::InputSimple(InputKind::Filter) => EventExec::exec_filter(status.selected())?,
            Mode::InputCompleted(CompletionKind::Exec) => EventExec::exec_exec(status.selected())?,
            Mode::InputCompleted(CompletionKind::Search) => {
                EventExec::exec_search(status.selected())
            }
            Mode::InputCompleted(CompletionKind::Goto) => EventExec::exec_goto(status.selected())?,
            Mode::NeedConfirmation(_)
            | Mode::Preview
            | Mode::InputCompleted(CompletionKind::Nothing)
            | Mode::InputSimple(InputKind::Sort)
            | Mode::InputSimple(InputKind::Marks(_)) => (),
            Mode::InputCompleted(_) => {