use std::os::unix::fs::MetadataExt;

use anyhow::Result;
use clap::Parser;
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::app::Tab;
use crate::common::{index_from_a, INPUT_HISTORY_PATH};
use crate::config::Bindings;
use crate::io::{drop_sudo_privileges, InputHistory, OpendalContainer};
use crate::io::{Args, DrawMenu};
use crate::log_line;
use crate::modes::{
    nvim_inform_ipc, Bulk, CliApplications, Completion, Compresser, ContentWindow, ContextMenu,
    Flagged, History, Input, InputCompleted, IsoDevice, Marks, Menu, Mount, Navigate,
    NeedConfirmation, NvimIPCAction, PasswordHolder, Picker, Remote, Selectable, Shortcut,
    TempMarks, Trash, TuiApplications, MAX_FILE_MODE,
};

macro_rules! impl_navigate_from_char {
    ($name:ident, $field:ident) => {
        #[doc = concat!(
         "Navigates to the index in the `",
         stringify!($field),
         "` field based on the given character."
                                                                                                )]
        pub fn $name(&mut self, c: char) -> bool {
            let Some(index) = index_from_a(c) else {
                return false;
            };
            if index < self.$field.len() {
                self.$field.set_index(index);
                self.window.scroll_to(index);
                return true;
            }
            false
        }
    };
}

/// Holds almost every menu except for the history, which is tab specific.
/// Only one instance is created and hold by status.
/// It acts as an interface for basic methods (navigation, length, completion) etc.
/// it also keeps track of the content window and user input (in menus only, not for the fuzzy finder).
///
/// The poor choices of architecture forced the creation of such a monster.
/// For instance, even if you never use marks or cloud, their instance is saved here,
/// waisting ressources.
///
/// Building them lazylly is on the todo list.
pub struct MenuHolder {
    /// Window for scrollable menus
    pub window: ContentWindow,
    /// Bulk rename
    pub bulk: Bulk,
    /// CLI applications
    pub cli_applications: CliApplications,
    /// cloud
    pub cloud: OpendalContainer,
    /// Completion list and index in it.
    pub completion: Completion,
    /// Compression methods
    pub compression: Compresser,
    /// Cotext menu
    pub context: ContextMenu,
    /// The flagged files
    pub flagged: Flagged,
    /// The typed input by the user
    pub input: Input,
    /// The user input history.
    pub input_history: InputHistory,
    /// Iso mounter. Set to None by default, dropped ASAP
    pub iso_device: Option<IsoDevice>,
    /// Marks allows you to jump to a save mark
    pub marks: Marks,
    /// Temporary marks allows you to jump to a save mark
    pub temp_marks: TempMarks,
    /// Hold password between their typing and usage
    pub password_holder: PasswordHolder,
    /// basic picker
    pub picker: Picker,
    /// Predefined shortcuts
    pub shortcut: Shortcut,
    /// TUI application
    pub tui_applications: TuiApplications,
    /// The trash
    pub trash: Trash,
    /// Last sudo command ran
    pub sudo_command: Option<String>,
    /// History - here for compatibility reasons only
    pub history: History,
    /// mounts
    pub mount: Mount,
}

impl MenuHolder {
    pub fn new(start_dir: &std::path::Path, binds: &Bindings) -> Result<Self> {
        Ok(Self {
            bulk: Bulk::default(),
            cli_applications: CliApplications::default(),
            cloud: OpendalContainer::default(),
            completion: Completion::default(),
            compression: Compresser::default(),
            context: ContextMenu::default(),
            flagged: Flagged::default(),
            history: History::default(),
            input: Input::default(),
            input_history: InputHistory::load(INPUT_HISTORY_PATH)?,
            iso_device: None,
            marks: Marks::default(),
            password_holder: PasswordHolder::default(),
            picker: Picker::default(),
            shortcut: Shortcut::empty(start_dir),
            sudo_command: None,
            temp_marks: TempMarks::default(),
            trash: Trash::new(binds)?,
            tui_applications: TuiApplications::default(),
            window: ContentWindow::default(),
            mount: Mount::default(),
        })
    }

    pub fn reset(&mut self) {
        self.input.reset();
        self.completion.reset();
        self.bulk.reset();
        self.sudo_command = None;
    }

    pub fn resize(&mut self, menu_mode: Menu, height: usize) {
        self.window.set_height(height);
        if let Menu::Navigate(_) = menu_mode {
            self.window.scroll_to(self.index(menu_mode))
        }
    }

    /// Replace the current input by the permission of the first flagged file as an octal value.
    /// If the flagged has permission "rwxrw.r.." or 764 in octal, "764" will be the current input.
    /// Only the last 3 octal digits are kept.
    /// Nothing is done if :
    /// - there's no flagged file,
    /// - can't read the metadata of the first flagged file.
    ///
    /// It should never happen.
    pub fn replace_input_by_permissions(&mut self) {
        let Some(flagged) = &self.flagged.content.first() else {
            return;
        };
        let Ok(metadata) = flagged.metadata() else {
            return;
        };
        let mode = metadata.mode() & MAX_FILE_MODE;
        self.input.replace(&format!("{mode:o}"));
    }

    /// Fill the input string with the currently selected completion.
    pub fn input_complete(&mut self, tab: &mut Tab) -> Result<()> {
        self.fill_completion(tab);
        self.window.reset(self.completion.len());
        Ok(())
    }

    fn fill_completion(&mut self, tab: &mut Tab) {
        match tab.menu_mode {
            Menu::InputCompleted(InputCompleted::Cd) => self.completion.cd(
                tab.current_directory_path()
                    .as_os_str()
                    .to_string_lossy()
                    .as_ref(),
                &self.input.string(),
            ),
            Menu::InputCompleted(InputCompleted::Exec) => {
                self.completion.exec(&self.input.string())
            }
            Menu::InputCompleted(InputCompleted::Search) => {
                self.completion.search(tab.completion_search_files());
            }
            Menu::InputCompleted(InputCompleted::Action) => {
                self.completion.action(&self.input.string())
            }
            _ => (),
        }
    }

    /// Run sshfs with typed parameters to mount a remote directory in current directory.
    /// sshfs should be reachable in path.
    /// The user must type 3 arguments like this : `username hostname remote_path`.
    /// If the user doesn't provide 3 arguments,
    pub fn mount_remote(&mut self, current_path: &str) {
        let input = self.input.string();
        if let Some(remote_builder) = Remote::from_input(input, current_path) {
            remote_builder.mount();
        }
        self.input.reset();
    }

    /// Remove a flag file from Jump mode
    pub fn remove_selected_flagged(&mut self) -> Result<()> {
        self.flagged.remove_selected();
        Ok(())
    }

    pub fn trash_delete_permanently(&mut self) -> Result<()> {
        self.trash.delete_permanently()
    }

    /// Delete all the flagged files & directory recursively.
    /// If an output socket was provided at launch, it will inform the IPC server about those deletions.
    /// Clear the flagged files.
    ///
    /// # Errors
    ///
    /// May fail if any deletion is impossible (permissions, file already deleted etc.)
    pub fn delete_flagged_files(&mut self) -> Result<()> {
        let nb = self.flagged.len();
        let output_socket = Args::parse().output_socket;
        while let Some(pathbuf) = self.flagged.content.pop() {
            if pathbuf.is_dir() {
                std::fs::remove_dir_all(&pathbuf)?;
            } else {
                std::fs::remove_file(&pathbuf)?;
            }
            if let Some(output_socket) = &output_socket {
                nvim_inform_ipc(output_socket, NvimIPCAction::DELETE(&pathbuf))?;
            }
            self.delete_mark(&pathbuf)?;
        }
        log_line!("Deleted {nb} flagged files");
        Ok(())
    }

    /// Reset the password holder, drop the sudo privileges (sudo -k) and clear the sudo command.
    pub fn clear_sudo_attributes(&mut self) -> Result<()> {
        self.password_holder.reset();
        drop_sudo_privileges()?;
        self.sudo_command = None;
        Ok(())
    }

    /// Insert a char in the input string.
    pub fn input_insert(&mut self, char: char) -> Result<()> {
        self.input.insert(char);
        Ok(())
    }

    /// Refresh the shortcuts. It drops non "hardcoded" shortcuts and
    /// extend the vector with the mount points.
    pub fn refresh_shortcuts(
        &mut self,
        mount_points: &[&std::path::Path],
        left_path: &std::path::Path,
        right_path: &std::path::Path,
    ) {
        self.shortcut.refresh(mount_points, left_path, right_path)
    }

    pub fn completion_reset(&mut self) {
        self.completion.reset();
    }

    pub fn completion_tab(&mut self) {
        self.input.replace(self.completion.current_proposition())
    }

    pub fn len(&self, menu_mode: Menu) -> usize {
        match menu_mode {
            Menu::Navigate(navigate) => self.apply_method(navigate, |variant| variant.len()),
            Menu::InputCompleted(_) => self.completion.len(),
            Menu::NeedConfirmation(need_confirmation) if need_confirmation.use_flagged_files() => {
                self.flagged.len()
            }
            Menu::NeedConfirmation(NeedConfirmation::EmptyTrash) => self.trash.len(),
            Menu::NeedConfirmation(NeedConfirmation::BulkAction) => self.bulk.len(),
            _ => 0,
        }
    }

    pub fn index(&self, menu_mode: Menu) -> usize {
        match menu_mode {
            Menu::Navigate(navigate) => self.apply_method(navigate, |variant| variant.index()),
            Menu::InputCompleted(_) => self.completion.index,
            Menu::NeedConfirmation(need_confirmation) if need_confirmation.use_flagged_files() => {
                self.flagged.index()
            }
            Menu::NeedConfirmation(NeedConfirmation::EmptyTrash) => self.trash.index(),
            Menu::NeedConfirmation(NeedConfirmation::BulkAction) => self.bulk.index(),
            _ => 0,
        }
    }

    pub fn page_down(&mut self, navigate: Navigate) {
        for _ in 0..10 {
            self.next(navigate)
        }
    }

    pub fn page_up(&mut self, navigate: Navigate) {
        for _ in 0..10 {
            self.prev(navigate)
        }
    }

    pub fn completion_prev(&mut self, input_completed: InputCompleted) {
        self.completion.prev();
        self.window
            .scroll_to(self.index(Menu::InputCompleted(input_completed)));
    }

    pub fn completion_next(&mut self, input_completed: InputCompleted) {
        self.completion.next();
        self.window
            .scroll_to(self.index(Menu::InputCompleted(input_completed)));
    }

    pub fn next(&mut self, navigate: Navigate) {
        self.apply_method_mut(navigate, |variant| variant.next());
        self.window.scroll_to(self.index(Menu::Navigate(navigate)));
    }

    pub fn prev(&mut self, navigate: Navigate) {
        self.apply_method_mut(navigate, |variant| variant.prev());
        self.window.scroll_to(self.index(Menu::Navigate(navigate)));
    }

    pub fn set_index(&mut self, index: usize, navigate: Navigate) {
        self.apply_method_mut(navigate, |variant| variant.set_index(index));
        self.window.scroll_to(self.index(Menu::Navigate(navigate)))
    }

    pub fn select_last(&mut self, navigate: Navigate) {
        let index = self.len(Menu::Navigate(navigate)).saturating_sub(1);
        self.set_index(index, navigate);
    }

    fn apply_method_mut<F, T>(&mut self, navigate: Navigate, func: F) -> T
    where
        F: FnOnce(&mut dyn Selectable) -> T,
    {
        match navigate {
            Navigate::CliApplication => func(&mut self.cli_applications),
            Navigate::Compress => func(&mut self.compression),
            Navigate::Mount => func(&mut self.mount),
            Navigate::Context => func(&mut self.context),
            Navigate::History => func(&mut self.history),
            Navigate::Marks(_) => func(&mut self.marks),
            Navigate::TempMarks(_) => func(&mut self.temp_marks),
            Navigate::Shortcut => func(&mut self.shortcut),
            Navigate::Trash => func(&mut self.trash),
            Navigate::TuiApplication => func(&mut self.tui_applications),
            Navigate::Cloud => func(&mut self.cloud),
            Navigate::Picker => func(&mut self.picker),
            Navigate::Flagged => func(&mut self.flagged),
        }
    }

    fn apply_method<F, T>(&self, navigate: Navigate, func: F) -> T
    where
        F: FnOnce(&dyn Selectable) -> T,
    {
        match navigate {
            Navigate::CliApplication => func(&self.cli_applications),
            Navigate::Compress => func(&self.compression),
            Navigate::Mount => func(&self.mount),
            Navigate::Context => func(&self.context),
            Navigate::History => func(&self.history),
            Navigate::Marks(_) => func(&self.marks),
            Navigate::TempMarks(_) => func(&self.temp_marks),
            Navigate::Shortcut => func(&self.shortcut),
            Navigate::Trash => func(&self.trash),
            Navigate::TuiApplication => func(&self.tui_applications),
            Navigate::Cloud => func(&self.cloud),
            Navigate::Picker => func(&self.picker),
            Navigate::Flagged => func(&self.flagged),
        }
    }

    // TODO! ensure it's displayed
    /// Draw a navigation menu with its simple `draw_menu` method.
    ///
    /// # Errors
    ///
    /// Some mode can't be displayed directly and this method will raise an error.
    /// It's the responsability of the caller to check beforehand.
    pub fn draw_navigate(&self, f: &mut Frame, rect: &Rect, navigate: Navigate) {
        match navigate {
            Navigate::Compress => self.compression.draw_menu(f, rect, &self.window),
            Navigate::Shortcut => self.shortcut.draw_menu(f, rect, &self.window),
            Navigate::Marks(_) => self.marks.draw_menu(f, rect, &self.window),
            Navigate::TuiApplication => self.tui_applications.draw_menu(f, rect, &self.window),
            Navigate::CliApplication => self.cli_applications.draw_menu(f, rect, &self.window),
            Navigate::Mount => self.mount.draw_menu(f, rect, &self.window),
            _ => unreachable!("{navigate} requires more information to be displayed."),
        }
    }

    /// Replace the current input by the next proposition from history
    /// for this edit mode.
    pub fn input_history_next(&mut self, tab: &mut Tab) -> Result<()> {
        if !self.input_history.is_mode_logged(&tab.menu_mode) {
            return Ok(());
        }
        self.input_history.next();
        self.input_history_replace(tab)
    }

    /// Replace the current input by the previous proposition from history
    /// for this edit mode.
    pub fn input_history_prev(&mut self, tab: &mut Tab) -> Result<()> {
        if !self.input_history.is_mode_logged(&tab.menu_mode) {
            return Ok(());
        }
        self.input_history.prev();
        self.input_history_replace(tab)
    }

    fn input_history_replace(&mut self, tab: &mut Tab) -> Result<()> {
        let Some(history_element) = self.input_history.current() else {
            return Ok(());
        };
        self.input.replace(history_element.content());
        self.input_complete(tab)?;
        Ok(())
    }

    fn delete_mark(&mut self, old_path: &std::path::Path) -> Result<()> {
        crate::log_info!("Remove mark {old_path:?}");
        self.temp_marks.remove_path(old_path);
        self.marks.remove_path(old_path)
    }

    impl_navigate_from_char!(shortcut_from_char, shortcut);
    impl_navigate_from_char!(context_from_char, context);
    impl_navigate_from_char!(tui_applications_from_char, tui_applications);
    impl_navigate_from_char!(cli_applications_from_char, cli_applications);
    impl_navigate_from_char!(compression_method_from_char, compression);
}
