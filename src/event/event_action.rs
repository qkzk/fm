    help_string, lsblk_and_cryptsetup_installed, open_tui_program, ContentWindow, Display,
    InputCompleted, InputSimple, LeaveMenu, MarkAction, Menu, Navigate, NeedConfirmation,
    PreviewBuilder, RemovableDevices, Search, Selectable,
            status.reset_menu_mode()?;
        if status.focus.is_file() && status.current_tab().display_mode.is_preview() {
            status.leave_preview()?;
        }
        if matches!(status.current_tab().menu_mode, Menu::Nothing) {
            return Ok(());
        };
        status.leave_menu_mode()?;
            status.set_edit_mode(1, Menu::Nothing)?;
        let edit_mode = &status.current_tab().menu_mode;
        if matches!(edit_mode, Menu::Navigate(Navigate::Flagged)) {
            status.leave_menu_mode()?;
        } else if matches!(edit_mode, Menu::Nothing) {
            status.set_edit_mode(status.index, Menu::Navigate(Navigate::Flagged))?;
        if !status.focus.is_file() || status.current_tab().display_mode.is_preview() {
            status.current_tab().menu_mode,
            Menu::InputSimple(InputSimple::Rename)
            status.reset_menu_mode()?;
        status.set_edit_mode(status.index, Menu::InputSimple(InputSimple::Rename))?;
            status.current_tab().menu_mode,
            Menu::NeedConfirmation(NeedConfirmation::Copy)
            status.reset_menu_mode()?;
            status.current_tab().menu_mode,
            Menu::NeedConfirmation(NeedConfirmation::Move)
            status.reset_menu_mode()?;
        status.set_edit_mode(status.index, Menu::NeedConfirmation(copy_or_move))
            Menu::NeedConfirmation(NeedConfirmation::Delete),
            status.current_tab().menu_mode,
            Menu::InputSimple(InputSimple::Chmod)
            status.reset_menu_mode()?;
            status.current_tab().menu_mode,
            Menu::InputSimple(InputSimple::Newdir | InputSimple::Newfile)
            status.reset_menu_mode()?;
            status.set_edit_mode(status.index, Menu::InputSimple(input_kind))?;
        if tab.display_mode.is_tree() {
            status.current_tab().menu_mode,
            Menu::InputCompleted(InputCompleted::Exec)
            status.reset_menu_mode()?;
        status.set_edit_mode(status.index, Menu::InputCompleted(InputCompleted::Exec))
            status.current_tab().menu_mode,
            Menu::InputSimple(InputSimple::Sort)
            status.reset_menu_mode()?;
        status.set_height_for_edit_mode(status.index, Menu::Nothing)?;
        status.tabs[status.index].menu_mode = Menu::Nothing;
        let len = status.menu.len(Menu::Nothing);
        status.tabs[status.index].menu_mode = Menu::InputSimple(InputSimple::Sort);
            status.current_tab().menu_mode,
            Menu::InputSimple(InputSimple::Filter)
            status.reset_menu_mode()?;
            status.set_edit_mode(status.index, Menu::InputSimple(InputSimple::Filter))?;
            status.current_tab().menu_mode,
            Menu::InputCompleted(InputCompleted::Search)
            status.reset_menu_mode()?;
        status.set_edit_mode(status.index, Menu::InputCompleted(InputCompleted::Search))
            status.current_tab().menu_mode,
            Menu::InputSimple(InputSimple::RegexMatch)
            status.reset_menu_mode()?;
            status.set_edit_mode(status.index, Menu::InputSimple(InputSimple::RegexMatch))
            status.current_tab().menu_mode,
            Menu::InputCompleted(InputCompleted::Cd)
            status.reset_menu_mode()?;
            status.set_edit_mode(status.index, Menu::InputCompleted(InputCompleted::Cd))?;
            status.current_tab().menu_mode,
            Menu::InputSimple(InputSimple::Shell)
            status.reset_menu_mode()?;
        status.set_edit_mode(status.index, Menu::InputSimple(InputSimple::Shell))
            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::TuiApplication)
            status.reset_menu_mode()?;
            status.set_edit_mode(status.index, Menu::Navigate(Navigate::TuiApplication))?;
            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::CliApplication)
            status.reset_menu_mode()?;
            status.set_edit_mode(status.index, Menu::Navigate(Navigate::CliApplication))?;
            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::History)
            status.reset_menu_mode()?;
            status.set_edit_mode(status.index, Menu::Navigate(Navigate::History))?;
            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::Marks(MarkAction::New))
            status.reset_menu_mode()?;
                Menu::Navigate(Navigate::Marks(MarkAction::New)),
            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::Marks(MarkAction::Jump))
            status.reset_menu_mode()?;
                Menu::Navigate(Navigate::Marks(MarkAction::Jump)),
            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::Shortcut)
            status.reset_menu_mode()?;
            status.set_edit_mode(status.index, Menu::Navigate(Navigate::Shortcut))?;
            status.current_tab().menu_mode,
            Menu::InputSimple(InputSimple::SetNvimAddr)
            status.reset_menu_mode()?;
        status.set_edit_mode(status.index, Menu::InputSimple(InputSimple::SetNvimAddr))
        match status.current_tab().display_mode {
            Display::Directory => status.current_tab_mut().directory_search_next(),
            match tab.menu_mode {
                Menu::Nothing => Self::move_display_up(status)?,
                Menu::Navigate(Navigate::History) => tab.history.prev(),
                Menu::Navigate(navigate) => status.menu.prev(navigate),
                Menu::InputCompleted(input_completed) => {
        if status.current_tab().display_mode.is_tree() && status.focus.is_file() {
        if status.current_tab().display_mode.is_tree() && status.focus.is_file() {
            match status.current_tab_mut().menu_mode {
                Menu::Nothing => Self::move_display_down(status)?,
                Menu::Navigate(Navigate::History) => status.current_tab_mut().history.next(),
                Menu::Navigate(navigate) => status.menu.next(navigate),
                Menu::InputCompleted(input_completed) => {
            match tab.menu_mode {
                Menu::InputSimple(_) | Menu::InputCompleted(_) => {
                Menu::Nothing => Self::file_move_left(tab)?,
                Menu::Navigate(Navigate::Cloud) => status.cloud_move_to_parent()?,
            match tab.menu_mode {
                Menu::InputSimple(_) | Menu::InputCompleted(_) => {
                Menu::Navigate(Navigate::Cloud) => status.cloud_enter_file_or_dir(),
                Menu::Nothing => Self::enter_file(status),
                LeaveMenu::leave_edit_mode(status, binds)?;
        match status.current_tab().menu_mode {
            Menu::Navigate(Navigate::Marks(MarkAction::New)) => {
            Menu::InputSimple(_) | Menu::InputCompleted(_) => {
            match status.current_tab_mut().menu_mode {
                Menu::InputSimple(_) | Menu::InputCompleted(_) => {
        match status.current_tab_mut().menu_mode {
            Menu::InputSimple(_) => {
            Menu::InputCompleted(_) => {
            match tab.menu_mode {
                Menu::Nothing => Self::file_page_up(status)?,
                Menu::Navigate(navigate) => status.menu.page_up(navigate),
                Menu::InputCompleted(input_completed) => {
            match tab.menu_mode {
                Menu::Nothing => Self::file_page_down(status)?,
                Menu::Navigate(navigate) => status.menu.page_down(navigate),
                Menu::InputCompleted(input_completed) => {
            LeaveMenu::leave_edit_mode(status, binds)
        } else if let Menu::InputCompleted(input_completed) = status.current_tab_mut().menu_mode {
            status.complete_tab(input_completed)?;
        // status.skim_output_to_tab();
        // status.skim_line_output_to_tab();
        // let help = help_string(binds, &status.internal_settings.opener);
        // status.skim_find_keybinding_and_run(help);
            status.current_tab().menu_mode,
            Menu::NeedConfirmation(NeedConfirmation::EmptyTrash)
            status.reset_menu_mode()?;
                Menu::NeedConfirmation(NeedConfirmation::EmptyTrash),
            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::Trash)
            status.reset_menu_mode()?;
            status.set_edit_mode(status.index, Menu::Navigate(Navigate::Trash))?;
    pub fn trash_restore(status: &mut Status) -> Result<()> {
        LeaveMenu::trash(status)
    }

            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::EncryptedDrive)
            status.reset_menu_mode()?;
            status.set_edit_mode(status.index, Menu::Navigate(Navigate::EncryptedDrive))?;
            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::RemovableDevices)
            status.reset_menu_mode()?;
            status.set_edit_mode(status.index, Menu::Navigate(Navigate::RemovableDevices))?;
            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::Compress)
            status.reset_menu_mode()?;
            status.set_edit_mode(status.index, Menu::Navigate(Navigate::Compress))?;
            status.current_tab().menu_mode,
            Menu::Navigate(Navigate::Context)
            status.reset_menu_mode()?;
            status.set_edit_mode(status.index, Menu::Navigate(Navigate::Context))?;
            status.current_tab().menu_mode,
            Menu::InputCompleted(InputCompleted::Action)
            status.reset_menu_mode()?;
            status.set_edit_mode(status.index, Menu::InputCompleted(InputCompleted::Action))?;
            status.current_tab().menu_mode,
            Menu::InputSimple(InputSimple::Remote)
            status.reset_menu_mode()?;
        status.set_edit_mode(status.index, Menu::InputSimple(InputSimple::Remote))
                if matches!(status.tabs[0].menu_mode, Menu::Nothing) {
                if matches!(status.tabs[1].menu_mode, Menu::Nothing) {
                if !matches!(status.tabs[0].menu_mode, Menu::Nothing) {
                if !matches!(status.tabs[1].menu_mode, Menu::Nothing) {