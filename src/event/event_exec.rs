use crate::modes::Edit;
use crate::modes::InputSimple;
use crate::modes::MarkAction;
use crate::modes::Navigate;
use crate::modes::NeedConfirmation;
        if matches!(status.current_tab().display_mode, Display::Preview) {
            status.tabs[status.index].set_display_mode(Display::Directory);
        status.menu.input.reset();
        status.menu.completion.reset();
        if status.reset_edit_mode()? {
            status.tabs[status.index].refresh_view()
            status.tabs[status.index].refresh_params()
        status.display_settings.toggle_metadata();
        status.display_settings.toggle_dual();
        status.display_settings.toggle_preview();
        if status.display_settings.preview() {
            status.set_edit_mode(1, Edit::Nothing)?;
            status.tabs[1].display_mode = Display::Directory;
    pub fn tree(status: &mut Status) -> Result<()> {
        status.current_tab_mut().toggle_tree_mode()?;
        status.refresh_view()
        tab.tree.toggle_fold(&tab.users);
        tab.tree.unfold_all(&tab.users);
        tab.tree.fold_all(&tab.users);
        status.set_edit_mode(status.index, Edit::InputSimple(InputSimple::Rename))
        status.set_edit_mode(status.index, Edit::NeedConfirmation(copy_or_move))
        status.set_edit_mode(
            status.index,
            Edit::NeedConfirmation(NeedConfirmation::Delete),
        )
    pub fn new_dir(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::InputSimple(InputSimple::Newdir))
    pub fn new_file(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::InputSimple(InputSimple::Newfile))
    pub fn exec(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::InputCompleted(InputCompleted::Exec))
    pub fn sort(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::InputSimple(InputSimple::Sort))
    pub fn filter(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::InputSimple(InputSimple::Filter))
            status.set_edit_mode(status.index, Edit::Navigate(Navigate::Jump))?;
        status.set_edit_mode(status.index, Edit::Navigate(Navigate::BulkMenu))
    pub fn search(status: &mut Status) -> Result<()> {
        let tab = status.current_tab_mut();
        status.set_edit_mode(status.index, Edit::InputCompleted(InputCompleted::Search))
    pub fn regex_match(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::InputSimple(InputSimple::RegexMatch))
        let help = help_string(binds, &status.internal_settings.opener);
        status.set_edit_mode(status.index, Edit::InputCompleted(InputCompleted::Goto))?;
    /// Enter the shell input command mode. The user can type a command which
    /// will be parsed and run.
    pub fn shell_command(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::InputSimple(InputSimple::Shell))
    pub fn tui_menu(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::Navigate(Navigate::TuiApplication))
        status.set_edit_mode(status.index, Edit::Navigate(Navigate::CliApplication))
    pub fn history(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::Navigate(Navigate::History))
    pub fn marks_new(status: &mut Status) -> Result<()> {
        status.set_edit_mode(
            status.index,
            Edit::Navigate(Navigate::Marks(MarkAction::New)),
        )
        status.set_edit_mode(
            status.index,
            Edit::Navigate(Navigate::Marks(MarkAction::Jump)),
        )
        status.set_edit_mode(status.index, Edit::Navigate(Navigate::Shortcut))

    /// Enter the set neovim RPC address mode where the user can type
    /// the RPC address himself
    pub fn set_nvim_server(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::InputSimple(InputSimple::SetNvimAddr))
        let home_path = path::Path::new(home);
        status.current_tab_mut().cd(home_path)?;
            Edit::Navigate(navigate) => status.menu.prev(navigate),
    pub fn next_sibling(tab: &mut Tab) -> Result<()> {
        if matches!(tab.display_mode, Display::Tree) {
            tab.tree_next_sibling();
        }
        Ok(())
    }

    pub fn previous_sibling(tab: &mut Tab) -> Result<()> {
        if matches!(tab.display_mode, Display::Tree) {
            tab.tree_prev_sibling();
        }
        Ok(())
    }

            Edit::Navigate(navigate) => status.menu.next(navigate),
        EventAction::click_file(status, row, col, binds)
    pub fn double_click(status: &mut Status, row: u16, col: u16, binds: &Bindings) -> Result<()> {
        if let Ok(()) = EventAction::click_file(status, row, col, binds) {
                tab.tree_page_down();
        let help = help_string(binds, &status.internal_settings.opener);
        status.set_edit_mode(
            status.index,
            Edit::NeedConfirmation(NeedConfirmation::EmptyTrash),
        )
        status.set_edit_mode(status.index, Edit::Navigate(Navigate::Trash))
        status.set_edit_mode(status.index, Edit::Navigate(Navigate::EncryptedDrive))
    /// Enter the Removable Devices mode where the user can mount an MTP device
        status.set_edit_mode(status.index, Edit::Navigate(Navigate::RemovableDevices))
        status.set_edit_mode(status.index, Edit::Navigate(Navigate::Compress))
    }

    /// Enter the context menu mode where the user can choose a basic file action.
    pub fn context(status: &mut Status) -> Result<()> {
        status.menu.context.reset();
        status.set_edit_mode(status.index, Edit::Navigate(Navigate::Context))
        status.set_edit_mode(status.index, Edit::InputCompleted(InputCompleted::Command))?;
    /// Enter the remote mount mode where the user can provide an username, an adress and
    /// a mount point to mount a remote device through SSHFS.
    pub fn remote_mount(status: &mut Status) -> Result<()> {
        status.set_edit_mode(status.index, Edit::InputSimple(InputSimple::Remote))
    /// Click a file at `row`, `col`.
    pub fn click_file(status: &mut Status, row: u16, col: u16, binds: &Bindings) -> Result<()> {
        status.click(row, col, binds)
    /// Select the left or right tab depending on `col`
    /// Execute Lazygit in a spawned terminal
    /// Execute NCDU in a spawned terminal