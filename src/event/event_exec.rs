use crate::common::LAZYGIT;
use crate::common::NCDU;
use crate::modes::Display;
use crate::modes::{Edit, InputSimple, MarkAction, Navigate, NeedConfirmation};
        if matches!(tab.edit_mode, Edit::Nothing) && !matches!(tab.display_mode, Display::Preview) {
        status.set_mode_chmod()
                .set_edit_mode(Edit::Navigate(Navigate::Jump))
        tab.set_edit_mode(Edit::Navigate(Navigate::Marks(MarkAction::New)));
            .set_edit_mode(Edit::Navigate(Navigate::Marks(MarkAction::Jump)));
            .set_edit_mode(Edit::Navigate(Navigate::Bulk));
        if matches!(tab.display_mode, Display::Preview) {
            tab.set_display_mode(Display::Normal);

        Self::set_copy_paste(status, NeedConfirmation::Copy)
        Self::set_copy_paste(status, NeedConfirmation::Move)
    }

    fn set_copy_paste(status: &mut Status, copy_or_move: NeedConfirmation) -> Result<()> {
            .set_edit_mode(Edit::NeedConfirmation(copy_or_move));
        tab.set_edit_mode(Edit::InputSimple(InputSimple::Newdir));
        tab.set_edit_mode(Edit::InputSimple(InputSimple::Newfile));
        tab.set_edit_mode(Edit::InputCompleted(InputCompleted::Exec));
            .set_edit_mode(Edit::NeedConfirmation(NeedConfirmation::Delete));
        status.selected().set_display_mode(Display::Preview);
        tab.set_display_mode(Display::Preview);
        tab.set_edit_mode(Edit::InputCompleted(InputCompleted::Search));
        if !matches!(tab.edit_mode, Edit::Nothing) {
        tab.set_edit_mode(Edit::InputSimple(InputSimple::RegexMatch));
        tab.set_edit_mode(Edit::InputSimple(InputSimple::Sort));
        if let Display::Tree = tab.display_mode {
        tab.set_edit_mode(Edit::InputCompleted(InputCompleted::Goto));
        tab.set_edit_mode(Edit::InputSimple(InputSimple::Shell));
        tab.set_edit_mode(Edit::Navigate(Navigate::ShellMenu));
            .set_edit_mode(Edit::Navigate(Navigate::CliInfo));
        tab.set_edit_mode(Edit::Navigate(Navigate::History));
        tab.set_edit_mode(Edit::Navigate(Navigate::Shortcut));
        tab.set_edit_mode(Edit::InputSimple(InputSimple::SetNvimAddr));
    /// See `crate::modes::Filter` for more details.
        tab.set_edit_mode(Edit::InputSimple(InputSimple::Filter));
        status.selected().cd(&path)?;
        status.selected().cd(&root_path)?;
        status.selected().cd(&start_folder)?;
            Display::Tree => tab.tree.search_first_match(&searched),
            Display::Normal => tab.normal_search_next(&searched),
            Display::Preview => {
            Edit::Nothing => Self::move_display_up(status)?,
            Edit::Navigate(Navigate::Jump) => status.flagged.prev(),
            Edit::Navigate(Navigate::History) => tab.history.prev(),
            Edit::Navigate(Navigate::Trash) => status.trash.prev(),
            Edit::Navigate(Navigate::Shortcut) => tab.shortcut.prev(),
            Edit::Navigate(Navigate::Marks(_)) => status.marks.prev(),
            Edit::Navigate(Navigate::Compress) => status.compression.prev(),
            Edit::Navigate(Navigate::Bulk) => status.bulk_prev(),
            Edit::Navigate(Navigate::ShellMenu) => status.shell_menu.prev(),
            Edit::Navigate(Navigate::CliInfo) => status.cli_info.prev(),
            Edit::Navigate(Navigate::EncryptedDrive) => status.encrypted_devices.prev(),
            Edit::InputCompleted(_) => tab.completion.prev(),
            Display::Normal => tab.normal_up_one_row(),
            Display::Preview => tab.preview_page_up(),
            Display::Tree => tab.tree_select_prev()?,
            Display::Normal => tab.normal_down_one_row(),
            Display::Preview => tab.preview_page_down(),
            Display::Tree => tab.tree_select_next()?,
            Edit::Nothing => Self::move_display_down(status)?,
            Edit::Navigate(Navigate::Jump) => status.flagged.next(),
            Edit::Navigate(Navigate::History) => status.selected().history.next(),
            Edit::Navigate(Navigate::Trash) => status.trash.next(),
            Edit::Navigate(Navigate::Shortcut) => status.selected().shortcut.next(),
            Edit::Navigate(Navigate::Marks(_)) => status.marks.next(),
            Edit::Navigate(Navigate::Compress) => status.compression.next(),
            Edit::Navigate(Navigate::Bulk) => status.bulk_next(),
            Edit::Navigate(Navigate::ShellMenu) => status.shell_menu.next(),
            Edit::Navigate(Navigate::CliInfo) => status.cli_info.next(),
            Edit::Navigate(Navigate::EncryptedDrive) => status.encrypted_devices.next(),
            Edit::InputCompleted(_) => status.selected().completion.next(),
            Edit::InputSimple(_) | Edit::InputCompleted(_) => {
            Edit::Nothing => match tab.display_mode {
                Display::Normal => tab.move_to_parent()?,
                Display::Tree => tab.tree_select_parent()?,
            Edit::InputSimple(_) | Edit::InputCompleted(_) => {
            Edit::Nothing => match tab.display_mode {
                Display::Normal => LeaveMode::open_file(status),
                Display::Tree => {
            Edit::InputSimple(_) | Edit::InputCompleted(_) => {
            Edit::InputSimple(_) | Edit::InputCompleted(_) => {
            Edit::Nothing => {
                    Display::Normal => tab.normal_go_top(),
                    Display::Preview => tab.preview_go_top(),
                    Display::Tree => tab.tree_go_to_root()?,
            Edit::Nothing => {
                    Display::Normal => tab.normal_go_bottom(),
                    Display::Preview => tab.preview_go_bottom(),
                    Display::Tree => tab.tree_go_to_bottom_leaf()?,
            Display::Normal => {
            Display::Preview => tab.preview_page_up(),
            Display::Tree => {
            Display::Normal => {
            Display::Preview => tab.preview_page_down(),
            Display::Tree => {
            Edit::InputSimple(InputSimple::Rename) => LeaveMode::rename(status.selected())?,
            Edit::InputSimple(InputSimple::Newfile) => LeaveMode::new_file(status.selected())?,
            Edit::InputSimple(InputSimple::Newdir) => LeaveMode::new_dir(status.selected())?,
            Edit::InputSimple(InputSimple::Chmod) => LeaveMode::chmod(status)?,
            Edit::InputSimple(InputSimple::RegexMatch) => LeaveMode::regex(status)?,
            Edit::InputSimple(InputSimple::SetNvimAddr) => LeaveMode::set_nvim_addr(status)?,
            Edit::InputSimple(InputSimple::Shell) => {
            Edit::InputSimple(InputSimple::Filter) => {
            Edit::InputSimple(InputSimple::Password(_, _)) => {
            Edit::InputSimple(InputSimple::Remote) => LeaveMode::remote(status.selected())?,
            Edit::Navigate(Navigate::Jump) => {
            Edit::Navigate(Navigate::History) => {
            Edit::Navigate(Navigate::Shortcut) => LeaveMode::shortcut(status)?,
            Edit::Navigate(Navigate::Trash) => LeaveMode::trash(status)?,
            Edit::Navigate(Navigate::Bulk) => LeaveMode::bulk(status)?,
            Edit::Navigate(Navigate::ShellMenu) => LeaveMode::shellmenu(status)?,
            Edit::Navigate(Navigate::CliInfo) => {
            Edit::Navigate(Navigate::EncryptedDrive) => (),
            Edit::Navigate(Navigate::Marks(MarkAction::New)) => LeaveMode::marks_update(status)?,
            Edit::Navigate(Navigate::Marks(MarkAction::Jump)) => LeaveMode::marks_jump(status)?,
            Edit::Navigate(Navigate::Compress) => LeaveMode::compress(status)?,
            Edit::Navigate(Navigate::RemovableDevices) => (),
            Edit::InputCompleted(InputCompleted::Exec) => LeaveMode::exec(status.selected())?,
            Edit::InputCompleted(InputCompleted::Search) => {
            Edit::InputCompleted(InputCompleted::Goto) => LeaveMode::goto(status)?,
            Edit::InputCompleted(InputCompleted::Command) => LeaveMode::command(status)?,
            Edit::NeedConfirmation(_)
            | Edit::InputCompleted(InputCompleted::Nothing)
            | Edit::InputSimple(InputSimple::Sort) => (),
            Edit::Nothing => match status.selected_non_mut().display_mode {
                Display::Normal => {
                    must_refresh = false;
                    LeaveMode::open_file(status)?;
                Display::Tree => LeaveMode::tree(status)?,
            Edit::InputCompleted(_) => {
            Edit::Nothing => status.next(),
        if matches!(status.selected_non_mut().edit_mode, Edit::Nothing) {
        status.skim_find_keybinding_and_run()
        if let Display::Normal | Display::Tree = tab.display_mode {
        if let Display::Normal | Display::Tree = tab.display_mode {
    pub fn resize(status: &mut Status, width: usize, height: usize) -> Result<()> {
        status.resize(width, height)
    }

        if let Display::Normal | Display::Tree = tab.display_mode {
            tab.set_display_mode(Display::Preview);
        if let Display::Normal | Display::Tree = status.selected_non_mut().display_mode {
            tab.set_display_mode(Display::Preview);
            .set_edit_mode(Edit::NeedConfirmation(NeedConfirmation::EmptyTrash));
            .set_edit_mode(Edit::Navigate(Navigate::Trash));
            .set_edit_mode(Edit::Navigate(Navigate::EncryptedDrive));
            .set_edit_mode(Edit::Navigate(Navigate::RemovableDevices));
            .set_edit_mode(Edit::Navigate(Navigate::Compress));
        tab.set_edit_mode(Edit::InputCompleted(InputCompleted::Command));
        tab.set_edit_mode(Edit::InputSimple(InputSimple::Remote));

    pub fn click_files(status: &mut Status, row: u16, col: u16) -> Result<()> {
        status.click(row, col)
    }

    pub fn select_pane(status: &mut Status, col: u16) -> Result<()> {
        status.select_pane(col)
    }

    pub fn click_first_line(col: u16, status: &mut Status) -> Result<()> {
        status.first_line_action(col)
    }

    pub fn lazygit(status: &mut Status) -> Result<()> {
        Self::open_program(status, LAZYGIT)
    }

    pub fn ncdu(status: &mut Status) -> Result<()> {
        Self::open_program(status, NCDU)
    }

    pub fn open_program(status: &mut Status, program: &str) -> Result<()> {
        if is_program_in_path(program) {
            crate::modes::ShellMenu::require_cwd_and_command(status, program)
        } else {
            Ok(())
        }
    }
        if matches!(tab.display_mode, Display::Tree) {
            tab.cd(path)?;
        status.selected().set_display_mode(Display::Preview);
        let original_path = if let Display::Tree = tab.display_mode {
            Display::Tree => {
        tab.cd(&path)?;
        tab.cd(&path)?;
        tab.cd(&path)?;
            status.selected().cd(&path)?;
            status.selected().set_display_mode(Display::Tree);
            Display::Normal => LeaveMode::open_file(status),
            Display::Tree => LeaveMode::tree(status),
        if let Display::Tree = tab.display_mode {