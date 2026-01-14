use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{
    mpsc::{self, Sender, TryRecvError},
    Arc,
};

use anyhow::{bail, Context, Result};
use clap::Parser;
use crossterm::event::{Event, KeyEvent};
use opendal::EntryMode;
use parking_lot::lock_api::Mutex;
use parking_lot::RawMutex;
use ratatui::layout::Size;
use sysinfo::Disks;
use walkdir::WalkDir;

use crate::app::{
    ClickableLine, Footer, Header, InternalSettings, Previewer, Session, Tab, ThumbnailManager,
};
use crate::common::{
    build_dest_path, current_username, disk_space, filename_from_path, is_in_path, is_sudo_command,
    path_to_string, row_to_window_index, set_current_dir, tilde, MountPoint,
};
use crate::config::{from_keyname, Bindings, START_FOLDER};
use crate::event::{ActionMap, FmEvents};
use crate::io::{
    build_tokio_greper, execute_and_capture_output, execute_sudo_command_with_password,
    execute_without_output, get_cloud_token_names, google_drive, reset_sudo_faillock, Args,
    Internal, Kind, Opener, MIN_WIDTH_FOR_DUAL_PANE,
};
use crate::modes::{
    append_files_to_shell_command, copy_move, parse_line_output, regex_flagger,
    shell_command_parser, Content, ContentWindow, CopyMove, CursorOffset,
    Direction as FuzzyDirection, Display, FileInfo, FileKind, FilterKind, FuzzyFinder, FuzzyKind,
    InputCompleted, InputSimple, IsoDevice, Menu, MenuHolder, MountAction, MountCommands,
    Mountable, Navigate, NeedConfirmation, PasswordKind, PasswordUsage, Permissions, PickerCaller,
    Preview, PreviewBuilder, ReEnterMenu, Search, Selectable, Users, SAME_WINDOW_TOKEN,
};
use crate::{log_info, log_line};

/// Kind of "windows" : Header, Files, Menu, Footer.
pub enum Window {
    Header,
    Files,
    Menu,
    Footer,
}

/// Responsible for switching the focus from one window to another.
#[derive(Default, Clone, Copy, Debug)]
pub enum Focus {
    #[default]
    LeftFile,
    LeftMenu,
    RightFile,
    RightMenu,
}

impl Focus {
    /// True if the focus is on left tab.
    /// Always true if only one tab is shown
    pub fn is_left(&self) -> bool {
        matches!(self, Self::LeftMenu | Self::LeftFile)
    }

    /// True if the focus is on a top window.
    /// Always true if no menu is shown.
    pub fn is_file(&self) -> bool {
        matches!(self, Self::LeftFile | Self::RightFile)
    }

    /// Switch from left to right and to file.
    /// `LeftMenu` -> `RightFile`
    /// `LeftFile` -> `RightFile`
    /// And vice versa.
    pub fn switch(&self) -> Self {
        match self {
            Self::LeftFile => Self::RightFile,
            Self::LeftMenu => Self::RightFile,
            Self::RightFile => Self::LeftFile,
            Self::RightMenu => Self::LeftFile,
        }
    }

    /// Returns the "parent" of current focus.
    /// File parent it itself (weird, I know), Menu parent is its associated file.
    /// Couldn't figure a better name.
    pub fn to_parent(&self) -> Self {
        match self {
            Self::LeftFile | Self::LeftMenu => Self::LeftFile,
            Self::RightFile | Self::RightMenu => Self::RightFile,
        }
    }

    /// In that order : `LeftFile, LeftMenu, RightFile, RightMenu`
    pub fn index(&self) -> usize {
        *self as usize
    }

    /// Index of the tab Left is 0 and Right is one.
    pub fn tab_index(&self) -> usize {
        self.index() >> 1
    }

    /// Is the window a left menu ?
    pub fn is_left_menu(&self) -> bool {
        matches!(self, Self::LeftMenu)
    }
}

/// What direction is used to sync tabs ? Left to right or right to left ?
pub enum Direction {
    RightToLeft,
    LeftToRight,
}

impl Direction {
    /// Returns the indexes of source and destination when tabs are synced
    const fn source_dest(self) -> (usize, usize) {
        match self {
            Self::RightToLeft => (1, 0),
            Self::LeftToRight => (0, 1),
        }
    }
}

/// Holds every mutable parameter of the application itself, except for
/// the "display" information.
/// It holds 2 tabs (left & right), even if only one can be displayed sometimes.
/// It knows which tab is selected, which files are flagged,
/// which jump target is selected, a cache of normal file colors,
/// if we have to display one or two tabs and if all details are shown or only
/// the filename.
/// Mutation of this struct are mostly done externally, by the event crate :
/// `crate::event_exec`.
pub struct Status {
    /// Vector of `Tab`, each of them are displayed in a separate tab.
    pub tabs: [Tab; 2],
    /// Index of the current selected tab
    pub index: usize,
    /// Fuzzy finder of files by name
    pub fuzzy: Option<FuzzyFinder<String>>,
    /// Navigable menu
    pub menu: MenuHolder,
    /// Display settings
    pub session: Session,
    /// Internal settings
    pub internal_settings: InternalSettings,
    /// Window being focused currently
    pub focus: Focus,
    /// Sender of events
    pub fm_sender: Arc<Sender<FmEvents>>,
    /// Receiver of previews, used to build & display previews without bloking
    preview_receiver: mpsc::Receiver<(PathBuf, Preview, usize)>,
    /// Non bloking preview builder
    pub previewer: Previewer,
    /// Preview manager
    pub thumbnail_manager: Option<ThumbnailManager>,
}

impl Status {
    /// Creates a new status for the application.
    /// It requires most of the information (arguments, configuration, height
    /// of the terminal, the formated help string).
    /// Status is wraped around by an `Arc`, `Mutex` from parking_lot.
    pub fn arc_mutex_new(
        size: Size,
        opener: Opener,
        binds: &Bindings,
        fm_sender: Arc<Sender<FmEvents>>,
    ) -> Result<Arc<Mutex<RawMutex, Self>>> {
        Ok(Arc::new(Mutex::new(Self::new(
            size, opener, binds, fm_sender,
        )?)))
    }

    fn new(
        size: Size,
        opener: Opener,
        binds: &Bindings,
        fm_sender: Arc<Sender<FmEvents>>,
    ) -> Result<Self> {
        let fuzzy = None;
        let index = 0;

        let args = Args::parse();
        let path = &START_FOLDER.get().context("Start folder should be set")?;
        let start_dir = if path.is_dir() {
            path
        } else {
            path.parent().context("")?
        };
        let disks = Disks::new_with_refreshed_list();
        let session = Session::new(size.width);
        let internal_settings = InternalSettings::new(opener, size, disks);
        let menu = MenuHolder::new(start_dir, binds)?;
        let focus = Focus::default();

        let users_left = Users::default();
        let users_right = users_left.clone();

        let height = size.height as usize;
        let tabs = [
            Tab::new(&args, height, users_left)?,
            Tab::new(&args, height, users_right)?,
        ];
        let (previewer_sender, preview_receiver) = mpsc::channel();
        let previewer = Previewer::new(previewer_sender);
        let thumbnail_manager = None;
        Ok(Self {
            tabs,
            index,
            fuzzy,
            menu,
            session,
            internal_settings,
            focus,
            fm_sender,
            preview_receiver,
            previewer,
            thumbnail_manager,
        })
    }

    /// Returns a non mutable reference to the selected tab.
    pub fn current_tab(&self) -> &Tab {
        &self.tabs[self.index]
    }

    /// Returns a mutable reference to the selected tab.
    pub fn current_tab_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.index]
    }

    /// Returns a string representing the current path in the selected tab.
    pub fn current_tab_path_str(&self) -> String {
        self.current_tab().directory_str()
    }

    /// True if a quit event was registered in the selected tab.
    pub fn must_quit(&self) -> bool {
        self.internal_settings.must_quit
    }

    /// Give the focus to the selected tab.
    pub fn focus_follow_index(&mut self) {
        if (self.index == 0 && !self.focus.is_left()) || (self.index == 1 && self.focus.is_left()) {
            self.focus = self.focus.switch();
        }
    }

    /// Give to focus to the left / right file / menu depending of which mode is selected.
    pub fn set_focus_from_mode(&mut self) {
        if self.index == 0 {
            if self.tabs[0].menu_mode.is_nothing() {
                self.focus = Focus::LeftFile;
            } else {
                self.focus = Focus::LeftMenu;
            }
        } else if self.tabs[1].menu_mode.is_nothing() {
            self.focus = Focus::RightFile;
        } else {
            self.focus = Focus::RightMenu;
        }
    }

    /// Select the other tab if two are displayed. Does nothing otherwise.
    pub fn next(&mut self) {
        if !self.session.dual() {
            return;
        }
        self.index = 1 - self.index;
        self.focus_follow_index();
    }

    /// Select the left or right tab depending on where the user clicked.
    pub fn select_tab_from_col(&mut self, col: u16) -> Result<()> {
        if self.session.dual() {
            if col < self.term_width() / 2 {
                self.select_left();
            } else {
                self.select_right();
            };
        } else {
            self.select_left();
        }
        Ok(())
    }

    fn window_from_row(&self, row: u16, height: u16) -> Window {
        let win_height = if self.current_tab().menu_mode.is_nothing() {
            height
        } else {
            height / 2
        };
        let w_index = row / win_height;
        if w_index == 1 {
            Window::Menu
        } else if row == 1 {
            Window::Header
        } else if row == win_height - 2 {
            Window::Footer
        } else {
            Window::Files
        }
    }

    /// Set focus from a mouse coordinates.
    /// When a mouse event occurs, focus is given to the window where it happened.
    pub fn set_focus_from_pos(&mut self, row: u16, col: u16) -> Result<Window> {
        self.select_tab_from_col(col)?;
        let window = self.window_from_row(row, self.term_size().height);
        self.set_focus_from_window_and_index(&window);
        Ok(window)
    }

    /// Execute a click at `row`, `col`. Action depends on which window was clicked.
    pub fn click(&mut self, binds: &Bindings, row: u16, col: u16) -> Result<()> {
        let window = self.set_focus_from_pos(row, col)?;
        self.click_action_from_window(&window, row, col, binds)?;
        Ok(())
    }

    /// True iff user has clicked on a preview in second pane.
    fn has_clicked_on_second_pane_preview(&self) -> bool {
        self.session.dual() && self.session.preview() && self.index == 1
    }

    fn click_action_from_window(
        &mut self,
        window: &Window,
        row: u16,
        col: u16,
        binds: &Bindings,
    ) -> Result<()> {
        match window {
            Window::Header => self.header_action(col, binds),
            Window::Files => {
                if self.has_clicked_on_second_pane_preview() {
                    if let Preview::Tree(tree) = &self.tabs[1].preview {
                        let index = row_to_window_index(row) + self.tabs[1].window.top;
                        let path = &tree.path_from_index(index)?;
                        self.select_file_from_right_tree(path)?;
                    }
                } else {
                    self.tab_select_row(row)?;
                    self.update_second_pane_for_preview()?;
                }
                Ok(())
            }
            Window::Footer => self.footer_action(col, binds),
            Window::Menu => self.menu_action(row, col),
        }
    }

    /// Action when user press enter on a preview.
    /// If the preview is a tree in right tab, the left file will be selected in the left tab.
    /// Otherwise, we open the file if it exists in the file tree.
    pub fn enter_from_preview(&mut self) -> Result<()> {
        if let Preview::Tree(tree) = &self.tabs[1].preview {
            let index = tree.displayable().index();
            let path = &tree.path_from_index(index)?;
            self.select_file_from_right_tree(path)?;
        } else {
            let filepath = self.tabs[self.focus.tab_index()].preview.filepath();
            if filepath.exists() {
                self.open_single_file(&filepath)?;
            }
        }
        Ok(())
    }

    fn select_file_from_right_tree(&mut self, path: &Path) -> Result<()> {
        self.tabs[0].cd_to_file(path)?;
        self.index = 0;
        self.focus = Focus::LeftFile;
        self.update_second_pane_for_preview()
    }

    /// Select a given row, if there's something in it.
    /// Returns an error if the clicked row is above the headers margin.
    pub fn tab_select_row(&mut self, row: u16) -> Result<()> {
        match self.current_tab().display_mode {
            Display::Directory => self.current_tab_mut().normal_select_row(row),
            Display::Tree => self.current_tab_mut().tree_select_row(row)?,
            Display::Fuzzy => self.fuzzy_navigate(FuzzyDirection::Index(row))?,
            _ => (),
        }
        Ok(())
    }

    #[rustfmt::skip]
    fn set_focus_from_window_and_index(&mut self, window: &Window) {
        self.focus = match (self.index == 0, window) {
            (true,  Window::Menu)   => Focus::LeftMenu,
            (true,  _)              => Focus::LeftFile,
            (false, Window::Menu)   => Focus::RightMenu,
            (false, _)              => Focus::RightFile
        };
    }

    /// Sync right tab from left tab path or vice versa.
    pub fn sync_tabs(&mut self, direction: Direction) -> Result<()> {
        let (source, dest) = direction.source_dest();
        self.tabs[dest].cd(&self.tabs[source].current_file()?.path)
    }

    /// Height of the second window (menu).
    /// Watchout : it's always ~height / 2 - 2, even if menu is closed.
    pub fn second_window_height(&self) -> Result<usize> {
        let height = self.term_size().height;
        Ok((height / 2).saturating_sub(2) as usize)
    }

    /// Execute a click on a menu item. Action depends on which menu was opened.
    fn menu_action(&mut self, row: u16, col: u16) -> Result<()> {
        let second_window_height = self.second_window_height()?;
        let row_offset = (row as usize).saturating_sub(second_window_height);
        const OFFSET: usize =
            ContentWindow::WINDOW_PADDING + ContentWindow::WINDOW_MARGIN_TOP_U16 as usize;
        if row_offset >= OFFSET {
            let index = row_offset - OFFSET + self.menu.window.top;
            match self.current_tab().menu_mode {
                Menu::Navigate(navigate) => match navigate {
                    Navigate::History => self.current_tab_mut().history.set_index(index),
                    navigate => self.menu.set_index(index, navigate),
                },
                Menu::InputCompleted(input_completed) => {
                    self.menu.completion.set_index(index);
                    if matches!(input_completed, InputCompleted::Search) {
                        self.follow_search()?;
                    }
                }
                Menu::NeedConfirmation(need_confirmation)
                    if need_confirmation.use_flagged_files() =>
                {
                    self.menu.flagged.set_index(index)
                }
                Menu::NeedConfirmation(NeedConfirmation::EmptyTrash) => {
                    self.menu.trash.set_index(index)
                }
                Menu::NeedConfirmation(NeedConfirmation::BulkAction) => {
                    self.menu.bulk.set_index(index)
                }
                _ => (),
            }
            self.menu.window.scroll_to(index);
        } else if row_offset == 3 && self.current_tab().menu_mode.is_input() {
            let index =
                col.saturating_sub(self.current_tab().menu_mode.cursor_offset() + 1) as usize;
            self.menu.input.cursor_move(index);
        }

        Ok(())
    }

    /// Select the left tab
    pub fn select_left(&mut self) {
        self.index = 0;
        self.focus_follow_index();
    }

    /// Select the right tab
    pub fn select_right(&mut self) {
        self.index = 1;
        self.focus_follow_index();
    }

    /// Refresh every disk information.
    /// It also refreshes the disk list, which is usefull to detect removable medias.
    /// It may be very slow...
    /// There's surelly a better way, like doing it only once in a while or on
    /// demand.
    pub fn refresh_shortcuts(&mut self) {
        self.menu.refresh_shortcuts(
            &self.internal_settings.mount_points_vec(),
            self.tabs[0].current_path(),
            self.tabs[1].current_path(),
        );
    }

    /// Returns the disk spaces for the selected tab..
    pub fn disk_spaces_of_selected(&self) -> String {
        disk_space(
            &self.internal_settings.disks,
            self.current_tab().current_path(),
        )
    }

    /// Returns the size of the terminal (width, height)
    pub fn term_size(&self) -> Size {
        self.internal_settings.term_size()
    }

    /// Returns the width of the terminal window.
    pub fn term_width(&self) -> u16 {
        self.term_size().width
    }

    /// Clears the right preview
    pub fn clear_preview_right(&mut self) {
        if self.session.dual() && self.session.preview() && !self.tabs[1].preview.is_empty() {
            self.tabs[1].preview = PreviewBuilder::empty()
        }
    }

    /// Refresh the current view, reloading the files. Move the selection to top.
    pub fn refresh_view(&mut self) -> Result<()> {
        self.refresh_status()?;
        self.update_second_pane_for_preview()
    }

    /// Reset the view of every tab.
    pub fn reset_tabs_view(&mut self) -> Result<()> {
        for tab in self.tabs.iter_mut() {
            match tab.refresh_and_reselect_file() {
                Ok(()) => (),
                Err(error) => log_info!("reset_tabs_view error: {error}"),
            }
        }
        Ok(())
    }

    /// Leave an edit mode and refresh the menu.
    /// It should only be called when edit mode isn't nothing.
    pub fn leave_menu_mode(&mut self) -> Result<()> {
        match self.current_tab().menu_mode {
            Menu::InputSimple(InputSimple::Filter) => {
                self.current_tab_mut().settings.reset_filter()
            }
            Menu::InputCompleted(InputCompleted::Cd) => self.current_tab_mut().cd_origin_path()?,
            _ => (),
        }
        if self.reset_menu_mode()? {
            self.current_tab_mut().refresh_view()?;
        } else {
            self.current_tab_mut().refresh_params();
        }
        self.current_tab_mut().reset_visual();
        Ok(())
    }

    /// Leave the preview or flagged display mode.
    /// Should only be called when :
    /// 1. No menu is opened
    ///
    /// AND
    ///
    /// 2. Display mode is preview or flagged.
    pub fn leave_preview(&mut self) -> Result<()> {
        self.current_tab_mut().set_display_mode(Display::Directory);
        self.current_tab_mut().refresh_and_reselect_file()
    }

    /// Reset the edit mode to "Nothing" (closing any menu) and returns
    /// true if the display should be refreshed.
    /// If the menu is a picker for input history, it will reenter the caller menu.
    /// Example: Menu CD -> History of CDs <Esc> -> Menu CD
    pub fn reset_menu_mode(&mut self) -> Result<bool> {
        if self.current_tab().menu_mode.is_picker() {
            if let Some(PickerCaller::Menu(menu)) = self.menu.picker.caller {
                menu.reenter(self)?;
                return Ok(false);
            }
        }
        self.menu.reset();
        let must_refresh = matches!(self.current_tab().display_mode, Display::Preview);
        self.set_menu_mode(self.index, Menu::Nothing)?;
        self.set_height_of_unfocused_menu()?;
        Ok(must_refresh)
    }

    fn set_height_of_unfocused_menu(&mut self) -> Result<()> {
        let unfocused_tab = &self.tabs[1 - self.index];
        match unfocused_tab.menu_mode {
            Menu::Nothing => (),
            unfocused_mode => {
                let len = self.menu.len(unfocused_mode);
                let height = self.second_window_height()?;
                self.menu.window = ContentWindow::new(len, height);
            }
        }
        Ok(())
    }

    /// Reset the selected tab view to the default.
    pub fn refresh_status(&mut self) -> Result<()> {
        self.force_clear();
        self.refresh_users()?;
        self.refresh_tabs()?;
        self.refresh_shortcuts();
        Ok(())
    }

    /// Set a "force clear" flag to true, which will reset the display.
    /// It's used when some command or whatever may pollute the terminal.
    /// We ensure to clear it before displaying again.
    pub fn force_clear(&mut self) {
        self.internal_settings.force_clear();
    }

    pub fn should_be_cleared(&self) -> bool {
        self.internal_settings.should_be_cleared()
    }

    /// Refresh the users for every tab
    pub fn refresh_users(&mut self) -> Result<()> {
        self.tabs[0].users.update();
        self.tabs[1].users = self.tabs[0].users.clone();
        Ok(())
    }

    /// Refresh the input, completion and every tab.
    pub fn refresh_tabs(&mut self) -> Result<()> {
        self.menu.input.reset();
        self.menu.completion.reset();
        self.tabs[0].refresh_and_reselect_file()?;
        self.tabs[1].refresh_and_reselect_file()
    }

    /// When a rezise event occurs, we may hide the second panel if the width
    /// isn't sufficiant to display enough information.
    /// We also need to know the new height of the terminal to start scrolling
    /// up or down.
    pub fn resize(&mut self, width: u16, height: u16) -> Result<()> {
        let couldnt_dual_but_want = self.couldnt_dual_but_want();
        self.internal_settings.update_size(width, height);
        if couldnt_dual_but_want {
            self.set_dual_pane_if_wide_enough()?;
        }
        if !self.wide_enough_for_dual() {
            self.select_left();
        }
        self.resize_all_windows(height)?;
        self.refresh_status()
    }

    fn wide_enough_for_dual(&self) -> bool {
        self.term_width() >= MIN_WIDTH_FOR_DUAL_PANE
    }

    fn couldnt_dual_but_want(&self) -> bool {
        !self.wide_enough_for_dual() && self.session.dual()
    }

    fn use_dual(&self) -> bool {
        self.wide_enough_for_dual() && self.session.dual()
    }

    fn left_window_width(&self) -> u16 {
        if self.use_dual() {
            self.term_width() / 2
        } else {
            self.term_width()
        }
    }

    fn resize_all_windows(&mut self, height: u16) -> Result<()> {
        let height_usize = height as usize;
        self.tabs[0].set_height(height_usize);
        self.tabs[1].set_height(height_usize);
        self.fuzzy_resize(height_usize);
        self.menu.resize(
            self.tabs[self.index].menu_mode,
            self.second_window_height()?,
        );
        Ok(())
    }

    /// Check if the second pane should display a preview and force it.
    pub fn update_second_pane_for_preview(&mut self) -> Result<()> {
        if self.are_settings_requiring_dualpane_preview() {
            if self.can_display_dualpane_preview() {
                self.set_second_pane_for_preview()?;
            } else {
                self.tabs[1].preview = PreviewBuilder::empty();
            }
        }
        Ok(())
    }

    fn are_settings_requiring_dualpane_preview(&self) -> bool {
        self.index == 0 && self.session.dual() && self.session.preview()
    }

    fn can_display_dualpane_preview(&self) -> bool {
        Session::display_wide_enough(self.term_width())
    }

    /// Force preview the selected file of the first pane in the second pane.
    /// Doesn't check if it has do.
    fn set_second_pane_for_preview(&mut self) -> Result<()> {
        self.tabs[1].set_display_mode(Display::Preview);
        self.tabs[1].menu_mode = Menu::Nothing;
        let Some(fileinfo) = self.get_correct_fileinfo_for_preview() else {
            return Ok(());
        };
        log_info!("sending preview request");
        self.previewer.build(fileinfo.path.to_path_buf(), 1)?;
        // self.preview_manager.enqueue(&fileinfo.path);

        Ok(())
    }

    /// Build all the video thumbnails of a directory
    /// Build the the thumbnail manager if it hasn't been initialised yet. If there's still files in the queue, they're cleared first.
    pub fn thumbnail_directory_video(&mut self) {
        if !self.are_settings_requiring_dualpane_preview() || !self.can_display_dualpane_preview() {
            return;
        }
        self.thumbnail_init_or_clear();
        let videos = self.current_tab().directory.videos();
        if videos.is_empty() {
            return;
        }
        if let Some(thumbnail_manager) = &self.thumbnail_manager {
            thumbnail_manager.enqueue(videos);
        }
    }

    /// Clear the thumbnail queue or init the manager.
    fn thumbnail_init_or_clear(&mut self) {
        if self.thumbnail_manager.is_none() {
            self.thumbnail_manager_init();
        } else {
            self.thumbnail_queue_clear();
        }
    }

    fn thumbnail_manager_init(&mut self) {
        self.thumbnail_manager = Some(ThumbnailManager::default());
    }

    /// Clear the thumbnail queue
    pub fn thumbnail_queue_clear(&self) {
        if let Some(thumbnail_manager) = &self.thumbnail_manager {
            thumbnail_manager.clear()
        }
    }

    /// Check if the previewer has sent a preview.
    ///
    /// If the previewer has sent a preview, it's attached to the correct tab.
    /// Returns an error if the previewer disconnected.
    /// Does nothing otherwise.
    pub fn check_preview(&mut self) -> Result<()> {
        match self.preview_receiver.try_recv() {
            Ok((path, preview, index)) => self.attach_preview(path, preview, index)?,
            Err(TryRecvError::Disconnected) => bail!("Previewer Disconnected"),
            Err(TryRecvError::Empty) => (),
        }
        Ok(())
    }

    /// Attach a preview to the correct tab.
    /// Nothing is done if the preview doesn't match the file.
    /// It may happen if the user navigates quickly with "heavy" previews (movies, large pdf, office documents etc.).
    fn attach_preview(&mut self, path: PathBuf, preview: Preview, index: usize) -> Result<()> {
        let compared_index = self.pick_correct_tab_from(index)?;
        if !self.preview_has_correct_path(compared_index, path.as_path())? {
            return Ok(());
        }
        self.tabs[index].preview = preview;
        self.tabs[index]
            .window
            .reset(self.tabs[index].preview.len());
        log_info!(
            "status: attach_preview {path} - {index}",
            path = path.display()
        );
        Ok(())
    }

    fn pick_correct_tab_from(&self, index: usize) -> Result<usize> {
        if index == 1 && self.can_display_dualpane_preview() && self.session.preview() {
            Ok(0)
        } else {
            Ok(index)
        }
    }

    /// Ok(true) if the preview should be used.
    /// We only check if one of 3 conditions are true :
    /// - are we in fuzzy mode in the compared_index tab ?
    /// - are we in navigate menu in the compared_index ?
    /// - is the path the current path ?
    fn preview_has_correct_path(&self, compared_index: usize, path: &Path) -> Result<bool> {
        let tab = &self.tabs[compared_index];
        Ok(tab.display_mode.is_fuzzy()
            || tab.menu_mode.is_navigate()
            || tab.current_file()?.path.as_ref() == path)
    }

    /// Look for the correct file_info to preview.
    /// It depends on what the left tab is doing :
    /// fuzzy mode ? its selection,
    /// navigation (shortcut, marks or history) ? the selection,
    /// otherwise, it's the current selection.
    fn get_correct_fileinfo_for_preview(&self) -> Option<FileInfo> {
        let left_tab = &self.tabs[0];
        let users = &left_tab.users;
        if left_tab.display_mode.is_fuzzy() {
            FileInfo::new(
                Path::new(&parse_line_output(&self.fuzzy_current_selection()?).ok()?),
                users,
            )
            .ok()
        } else if self.focus.is_left_menu() {
            self.fileinfo_from_navigate(left_tab, users)
        } else {
            left_tab.current_file().ok()
        }
    }

    /// FileInfo to be previewed depending of what is done in this tab.
    /// If this tab is navigating in history, shortcut or marks, we return the selection.
    /// Otherwise, we return the current file.
    fn fileinfo_from_navigate(&self, tab: &Tab, users: &Users) -> Option<FileInfo> {
        match tab.menu_mode {
            Menu::Navigate(Navigate::History) => {
                FileInfo::new(tab.history.content().get(tab.history.index())?, users).ok()
            }
            Menu::Navigate(Navigate::Shortcut) => {
                let short = &self.menu.shortcut;
                FileInfo::new(short.content().get(short.index())?, users).ok()
            }
            Menu::Navigate(Navigate::Marks(_)) => {
                let (_, mark_path) = &self.menu.marks.content().get(self.menu.marks.index())?;
                FileInfo::new(mark_path, users).ok()
            }
            Menu::Navigate(Navigate::Flagged) => {
                FileInfo::new(self.menu.flagged.selected()?, users).ok()
            }
            _ => tab.current_file().ok(),
        }
    }

    /// Does any tab set its flag for image to be cleared on next display ?
    pub fn should_tabs_images_be_cleared(&self) -> bool {
        self.tabs[0].settings.should_clear_image || self.tabs[1].settings.should_clear_image
    }

    /// Reset the tab flags to false. Called when their image have be cleared.
    pub fn set_tabs_images_cleared(&mut self) {
        self.tabs[0].settings.should_clear_image = false;
        self.tabs[1].settings.should_clear_image = false;
    }

    /// Set an edit mode for the tab at `index`. Refresh the view.
    pub fn set_menu_mode(&mut self, index: usize, menu_mode: Menu) -> Result<()> {
        self.set_menu_mode_no_refresh(index, menu_mode)?;
        self.current_tab_mut().reset_visual();
        self.refresh_status()
    }

    /// Set the menu and without querying a refresh.
    pub fn set_menu_mode_no_refresh(&mut self, index: usize, menu_mode: Menu) -> Result<()> {
        if index > 1 {
            return Ok(());
        }
        self.set_height_for_menu_mode(index, menu_mode)?;
        self.tabs[index].menu_mode = menu_mode;
        let len = self.menu.len(menu_mode);
        let height = self.second_window_height()?;
        self.menu.window = ContentWindow::new(len, height);
        self.menu.window.scroll_to(self.menu.index(menu_mode));
        self.set_focus_from_mode();
        self.menu.input_history.filter_by_mode(menu_mode);
        Ok(())
    }

    /// Set the height of the menu and scroll to the selected item.
    pub fn set_height_for_menu_mode(&mut self, index: usize, menu_mode: Menu) -> Result<()> {
        let height = self.term_size().height;
        let prim_window_height = if menu_mode.is_nothing() {
            height
        } else {
            height / 2
        };
        self.tabs[index]
            .window
            .set_height(prim_window_height as usize);
        self.tabs[index]
            .window
            .scroll_to(self.tabs[index].window.top);
        Ok(())
    }

    /// Set dual pane if the term is big enough
    pub fn set_dual_pane_if_wide_enough(&mut self) -> Result<()> {
        if self.wide_enough_for_dual() {
            self.session.set_dual(true);
        } else {
            self.select_left();
            self.session.set_dual(false);
        }
        Ok(())
    }

    /// Empty the flagged files, reset the view of every tab.
    pub fn clear_flags_and_reset_view(&mut self) -> Result<()> {
        self.menu.flagged.clear();
        self.reset_tabs_view()
    }

    /// Returns the pathes of flagged file or the selected file if nothing is flagged
    fn flagged_or_selected(&self) -> Vec<PathBuf> {
        if self.menu.flagged.is_empty() {
            let Ok(file) = self.current_tab().current_file() else {
                return vec![];
            };
            vec![file.path.to_path_buf()]
        } else {
            self.menu.flagged.content().to_owned()
        }
    }

    fn flagged_or_selected_relative_to(&self, here: &Path) -> Vec<PathBuf> {
        self.flagged_or_selected()
            .iter()
            .filter_map(|abs_path| pathdiff::diff_paths(abs_path, here))
            .filter(|f| !f.starts_with(".."))
            .collect()
    }

    /// Returns a vector of path of files which are both flagged and in current
    /// directory.
    /// It's necessary since the user may have flagged files OUTSIDE of current
    /// directory before calling Bulkrename.
    /// It may be confusing since the same filename can be used in
    /// different places.
    pub fn flagged_in_current_dir(&self) -> Vec<PathBuf> {
        self.menu.flagged.in_dir(&self.current_tab().directory.path)
    }

    /// Flag all files in the current directory or current tree.
    pub fn flag_all(&mut self) {
        match self.current_tab().display_mode {
            Display::Directory => {
                self.tabs[self.index]
                    .directory
                    .content
                    .iter()
                    .filter(|file| file.filename.as_ref() != "." && file.filename.as_ref() != "..")
                    .for_each(|file| {
                        self.menu.flagged.push(file.path.to_path_buf());
                    });
            }
            Display::Tree => self.tabs[self.index].tree.flag_all(&mut self.menu.flagged),
            _ => (),
        }
    }

    /// Reverse every flag in _current_ directory. Flagged files in other
    /// directory aren't affected.
    pub fn reverse_flags(&mut self) {
        if self.current_tab().display_mode.is_preview() {
            self.tabs[self.index]
                .directory
                .content
                .iter()
                .for_each(|file| self.menu.flagged.toggle(&file.path));
        }
    }

    /// Flag all _file_ (everything but directories) which are children of a directory.
    pub fn toggle_flag_for_children(&mut self) {
        let Ok(file) = self.current_tab().current_file() else {
            return;
        };
        match self.current_tab().display_mode {
            Display::Directory => self.toggle_flag_for_selected(),
            Display::Tree => {
                if !file.path.is_dir() {
                    self.menu.flagged.toggle(&file.path);
                } else {
                    for entry in WalkDir::new(&file.path).into_iter().filter_map(|e| e.ok()) {
                        let p = entry.path();
                        if !p.is_dir() {
                            self.menu.flagged.toggle(p);
                        }
                    }
                }
                let _ = self.update_second_pane_for_preview();
            }
            Display::Preview => (),
            Display::Fuzzy => (),
        }
    }
    /// Flag the selected file if any
    pub fn toggle_flag_for_selected(&mut self) {
        let Ok(file) = self.current_tab().current_file() else {
            return;
        };
        match self.current_tab().display_mode {
            Display::Directory => {
                self.menu.flagged.toggle(&file.path);
                if !self.current_tab().directory.selected_is_last() {
                    self.tabs[self.index].normal_down_one_row();
                }
                let _ = self.update_second_pane_for_preview();
            }
            Display::Tree => {
                self.menu.flagged.toggle(&file.path);
                if !self.current_tab().tree.selected_is_last() {
                    self.current_tab_mut().tree_select_next();
                }
                let _ = self.update_second_pane_for_preview();
            }
            Display::Preview => (),
            Display::Fuzzy => (),
        }
        if matches!(
            self.current_tab().menu_mode,
            Menu::Navigate(Navigate::Flagged)
        ) {
            self.menu.window.set_len(self.menu.flagged.len());
        }
    }

    /// Jumps to the selected flagged file.
    pub fn jump_flagged(&mut self) -> Result<()> {
        let Some(path) = self.menu.flagged.selected() else {
            return Ok(());
        };
        let path = path.to_owned();
        let tab = self.current_tab_mut();
        tab.set_display_mode(Display::Directory);
        tab.refresh_view()?;
        tab.jump(path)?;
        self.update_second_pane_for_preview()
    }

    /// Execute a move or a copy of the flagged files to current directory.
    /// A progress bar is displayed (invisible for small files) and a notification
    /// is sent every time, even for 0 bytes files...
    pub fn cut_or_copy_flagged_files(&mut self, cut_or_copy: CopyMove) -> Result<()> {
        let sources = self.menu.flagged.content.clone();
        let dest = &self.current_tab().directory_of_selected()?.to_owned();
        self.cut_or_copy_files(cut_or_copy, sources, dest)
    }

    fn cut_or_copy_files(
        &mut self,
        cut_or_copy: CopyMove,
        mut sources: Vec<PathBuf>,
        dest: &Path,
    ) -> Result<()> {
        if matches!(cut_or_copy, CopyMove::Move) {
            sources = Self::remove_subdir_of_dest(sources, dest);
            if sources.is_empty() {
                return Ok(());
            }
        }

        if self.is_simple_move(&cut_or_copy, &sources, dest) {
            self.simple_move(&sources, dest)
        } else {
            self.complex_move(cut_or_copy, sources, dest)
        }
    }

    /// Prevent the user from moving to a subdirectory of itself.
    /// Only used before _moving_, for copying it doesn't matter.
    fn remove_subdir_of_dest(mut sources: Vec<PathBuf>, dest: &Path) -> Vec<PathBuf> {
        for index in (0..sources.len()).rev() {
            if sources[index].starts_with(dest) {
                log_info!("Cannot move to a subdirectory of itself");
                log_line!("Cannot move to a subdirectory of itself");
                sources.remove(index);
            }
        }
        sources
    }

    /// True iff it's a _move_ (not a copy) where all sources share the same mountpoint as destination.
    fn is_simple_move(&self, cut_or_copy: &CopyMove, sources: &[PathBuf], dest: &Path) -> bool {
        if cut_or_copy.is_copy() || sources.is_empty() {
            return false;
        }
        let mount_points = self.internal_settings.mount_points_set();
        let Some(source_mount_point) = sources[0].mount_point(&mount_points) else {
            return false;
        };
        for source in sources {
            if source.mount_point(&mount_points) != Some(source_mount_point) {
                return false;
            }
        }
        Some(source_mount_point) == dest.mount_point(&mount_points)
    }

    fn simple_move(&mut self, sources: &[PathBuf], dest: &Path) -> Result<()> {
        for source in sources {
            let filename = filename_from_path(source)?;
            let dest = dest.to_path_buf().join(filename);
            log_info!("simple_move {source:?} -> {dest:?}");
            match std::fs::rename(source, &dest) {
                Ok(()) => {
                    log_line!(
                        "Moved {source} to {dest}",
                        source = source.display(),
                        dest = dest.display()
                    )
                }
                Err(e) => {
                    log_info!("Error: {e:?}");
                    log_line!("Error: {e:?}")
                }
            }
        }
        self.clear_flags_and_reset_view()
    }

    fn complex_move(
        &mut self,
        cut_or_copy: CopyMove,
        sources: Vec<PathBuf>,
        dest: &Path,
    ) -> Result<()> {
        let mut must_act_now = true;
        if matches!(cut_or_copy, CopyMove::Copy) {
            if !self.internal_settings.copy_file_queue.is_empty() {
                log_info!("cut_or_copy_flagged_files: act later");
                must_act_now = false;
            }
            self.internal_settings
                .copy_file_queue
                .push((sources.to_owned(), dest.to_path_buf()));
        }

        if must_act_now {
            log_info!("cut_or_copy_flagged_files: act now");
            let in_mem = copy_move(
                cut_or_copy,
                sources,
                dest,
                self.left_window_width(),
                self.term_size().height,
                Arc::clone(&self.fm_sender),
            )?;
            self.internal_settings.store_copy_progress(in_mem);
        }
        self.clear_flags_and_reset_view()
    }

    /// Copy the next file in copy queue
    pub fn copy_next_file_in_queue(&mut self) -> Result<()> {
        self.internal_settings
            .copy_next_file_in_queue(self.fm_sender.clone(), self.left_window_width())
    }

    /// Paste a text into an input box.
    pub fn paste_input(&mut self, pasted: &str) -> Result<()> {
        if self.focus.is_file() && self.current_tab().display_mode.is_fuzzy() {
            let Some(fuzzy) = &mut self.fuzzy else {
                return Ok(());
            };
            fuzzy.input.insert_string(pasted);
        } else if !self.focus.is_file() && self.current_tab().menu_mode.is_input() {
            self.menu.input.insert_string(pasted);
        }
        Ok(())
    }

    /// Paste a path a move or copy file in current directory.
    pub fn paste_pathes(&mut self, pasted: &str) -> Result<()> {
        for pasted in pasted.split_whitespace() {
            self.paste_path(pasted)?;
        }
        Ok(())
    }

    fn paste_path(&mut self, pasted: &str) -> Result<()> {
        // recognize pathes
        let pasted = Path::new(&pasted);
        if !pasted.is_absolute() {
            log_info!("pasted {pasted} isn't absolute.", pasted = pasted.display());
            return Ok(());
        }
        if !pasted.exists() {
            log_info!("pasted {pasted} doesn't exist.", pasted = pasted.display());
            return Ok(());
        }
        // build dest path
        let dest = self.current_tab().current_path().to_path_buf();
        let Some(dest_filename) = build_dest_path(pasted, &dest) else {
            return Ok(());
        };
        if dest_filename == pasted {
            log_info!("pasted is same directory.");
            return Ok(());
        }
        if dest_filename.exists() {
            log_info!(
                "pasted {dest_filename} already exists",
                dest_filename = dest_filename.display()
            );
            return Ok(());
        }
        let sources = vec![pasted.to_path_buf()];

        log_info!("pasted copy {sources:?} to {dest:?}");
        if self.is_simple_move(&CopyMove::Move, &sources, &dest) {
            self.simple_move(&sources, &dest)
        } else {
            self.complex_move(CopyMove::Copy, sources, &dest)
        }
    }

    /// Init the fuzzy finder
    pub fn fuzzy_init(&mut self, kind: FuzzyKind) {
        self.fuzzy = Some(FuzzyFinder::new(kind).set_height(self.current_tab().window.height));
    }

    fn fuzzy_drop(&mut self) {
        self.fuzzy = None;
    }

    /// Sets the fuzzy finder to find files
    pub fn fuzzy_find_files(&mut self) -> Result<()> {
        let Some(fuzzy) = &self.fuzzy else {
            bail!("Fuzzy should be set");
        };
        let current_path = self.current_tab().current_path().to_path_buf();
        fuzzy.find_files(current_path);
        Ok(())
    }

    /// Sets the fuzzy finder to match against help lines
    pub fn fuzzy_help(&mut self, help: String) -> Result<()> {
        let Some(fuzzy) = &self.fuzzy else {
            bail!("Fuzzy should be set");
        };
        fuzzy.find_action(help);
        Ok(())
    }

    /// Sets the fuzzy finder to match against text file content
    pub fn fuzzy_find_lines(&mut self) -> Result<()> {
        let Some(fuzzy) = &self.fuzzy else {
            bail!("Fuzzy should be set");
        };
        let Some(tokio_greper) = build_tokio_greper() else {
            log_info!("ripgrep & grep aren't in $PATH");
            return Ok(());
        };
        fuzzy.find_line(tokio_greper);
        Ok(())
    }

    fn fuzzy_current_selection(&self) -> Option<std::string::String> {
        if let Some(fuzzy) = &self.fuzzy {
            fuzzy.pick()
        } else {
            None
        }
    }

    /// Action when a fuzzy item is selected.
    /// It depends of the kind of fuzzy:
    /// files / line: go to this file,
    /// help: execute the selected action.
    pub fn fuzzy_select(&mut self) -> Result<()> {
        let Some(fuzzy) = &self.fuzzy else {
            bail!("Fuzzy should be set");
        };
        if let Some(pick) = fuzzy.pick() {
            match fuzzy.kind {
                FuzzyKind::File => self.tabs[self.index].cd_to_file(Path::new(&pick))?,
                FuzzyKind::Line => self.tabs[self.index].cd_to_file(&parse_line_output(&pick)?)?,
                FuzzyKind::Action => self.fuzzy_send_event(&pick)?,
            }
        } else {
            log_info!("Fuzzy had nothing to pick from");
        };
        self.fuzzy_leave()
    }

    pub fn fuzzy_toggle_flag_selected(&mut self) -> Result<()> {
        let Some(fuzzy) = &self.fuzzy else {
            bail!("Fuzzy should be set");
        };
        if let Some(pick) = fuzzy.pick() {
            if let FuzzyKind::File = fuzzy.kind {
                self.menu.flagged.toggle(Path::new(&pick))
            }
        } else {
            log_info!("Fuzzy had nothing to select from");
        };
        Ok(())
    }

    /// Run a command directly from help.
    /// Search a command with fuzzy finder, if it's a keybinding, run it directly.
    /// If the result can't be parsed, nothing is done.
    fn fuzzy_send_event(&self, pick: &str) -> Result<()> {
        if let Ok(key) = find_keybind_from_fuzzy(pick) {
            self.fm_sender.send(FmEvents::Term(Event::Key(key)))?;
        };
        Ok(())
    }

    /// Exits the fuzzy finder and drops its instance
    pub fn fuzzy_leave(&mut self) -> Result<()> {
        self.fuzzy_drop();
        self.current_tab_mut().set_display_mode(Display::Directory);
        self.refresh_view()
    }

    /// Deletes a char to the left in fuzzy finder.
    pub fn fuzzy_backspace(&mut self) -> Result<()> {
        let Some(fuzzy) = &mut self.fuzzy else {
            bail!("Fuzzy should be set");
        };
        fuzzy.input.delete_char_left();
        fuzzy.update_input(false);
        Ok(())
    }

    /// Delete all chars to the right in fuzzy finder.
    pub fn fuzzy_delete(&mut self) -> Result<()> {
        let Some(fuzzy) = &mut self.fuzzy else {
            bail!("Fuzzy should be set");
        };
        fuzzy.input.delete_chars_right();
        fuzzy.update_input(false);
        Ok(())
    }

    /// Move cursor to the left in fuzzy finder
    pub fn fuzzy_left(&mut self) -> Result<()> {
        let Some(fuzzy) = &mut self.fuzzy else {
            bail!("Fuzzy should be set");
        };
        fuzzy.input.cursor_left();
        Ok(())
    }

    /// Move cursor to the right in fuzzy finder
    pub fn fuzzy_right(&mut self) -> Result<()> {
        let Some(fuzzy) = &mut self.fuzzy else {
            bail!("Fuzzy should be set");
        };
        fuzzy.input.cursor_right();
        Ok(())
    }

    /// Move selection to the start (top)
    pub fn fuzzy_start(&mut self) -> Result<()> {
        self.fuzzy_navigate(FuzzyDirection::Start)
    }

    /// Move selection to the end (bottom)
    pub fn fuzzy_end(&mut self) -> Result<()> {
        self.fuzzy_navigate(FuzzyDirection::End)
    }

    /// Navigate to a [`FuzzyDirection`].
    pub fn fuzzy_navigate(&mut self, direction: FuzzyDirection) -> Result<()> {
        let Some(fuzzy) = &mut self.fuzzy else {
            bail!("Fuzzy should be set");
        };
        fuzzy.navigate(direction);
        if fuzzy.should_preview() {
            self.update_second_pane_for_preview()?;
        }
        Ok(())
    }

    /// Issue a tick to the fuzzy finder
    pub fn fuzzy_tick(&mut self) {
        match &mut self.fuzzy {
            Some(fuzzy) => {
                fuzzy.tick(false);
            }
            None => (),
        }
    }

    /// Resize the fuzzy finder according to given height
    pub fn fuzzy_resize(&mut self, height: usize) {
        match &mut self.fuzzy {
            Some(fuzzy) => fuzzy.resize(height),
            None => (),
        }
    }

    /// Replace the current input by the next result from history
    pub fn input_history_next(&mut self) -> Result<()> {
        if self.focus.is_file() {
            return Ok(());
        }
        self.menu.input_history_next(&mut self.tabs[self.index])?;
        if let Menu::InputCompleted(input_completed) = self.current_tab().menu_mode {
            self.complete(input_completed)?;
        }
        Ok(())
    }

    /// Replace the current input by the previous result from history
    pub fn input_history_prev(&mut self) -> Result<()> {
        if self.focus.is_file() {
            return Ok(());
        }
        self.menu.input_history_prev(&mut self.tabs[self.index])?;
        if let Menu::InputCompleted(input_completed) = self.current_tab().menu_mode {
            self.complete(input_completed)?;
        }
        Ok(())
    }

    /// Push the typed char `c` into the input string and fill the completion menu with results.
    pub fn input_and_complete(&mut self, input_completed: InputCompleted, c: char) -> Result<()> {
        self.menu.input.insert(c);
        self.complete(input_completed)
    }

    pub fn complete(&mut self, input_completed: InputCompleted) -> Result<()> {
        match input_completed {
            InputCompleted::Search => self.complete_search(),
            _ => self.complete_non_search(),
        }
    }

    /// Update the input string with the current selection and fill the completion menu with results.
    pub fn complete_tab(&mut self, input_completed: InputCompleted) -> Result<()> {
        self.menu.completion_tab();
        self.complete_cd_move()?;
        if matches!(input_completed, InputCompleted::Search) {
            self.update_search()?;
            self.search()?;
        } else {
            self.menu.input_complete(&mut self.tabs[self.index])?
        }
        Ok(())
    }

    fn complete_search(&mut self) -> Result<()> {
        self.update_search()?;
        self.search()?;
        self.menu.input_complete(&mut self.tabs[self.index])
    }

    fn update_search(&mut self) -> Result<()> {
        if let Ok(search) = Search::new(&self.menu.input.string()) {
            self.current_tab_mut().search = search;
        };
        Ok(())
    }

    /// Select the currently proposed item in search mode.
    /// This is usefull to allow the user to select an item when moving with up or down.
    pub fn follow_search(&mut self) -> Result<()> {
        let Some(proposition) = self.menu.completion.selected() else {
            return Ok(());
        };
        let current_path = match self.current_tab().display_mode {
            Display::Directory => self.current_tab().current_path(),
            Display::Tree => self.current_tab().tree.root_path(),
            _ => {
                return Ok(());
            }
        };
        let mut full_path = current_path.to_path_buf();
        full_path.push(proposition);
        self.current_tab_mut().select_by_path(Arc::from(full_path));
        Ok(())
    }

    fn complete_non_search(&mut self) -> Result<()> {
        self.complete_cd_move()?;
        self.menu.input_complete(&mut self.tabs[self.index])
    }

    /// Move to the input path if possible.
    pub fn complete_cd_move(&mut self) -> Result<()> {
        if let Menu::InputCompleted(InputCompleted::Cd) = self.current_tab().menu_mode {
            let input = self.menu.input.string();
            if self.tabs[self.index].try_cd_to_file(input)? {
                self.update_second_pane_for_preview()?;
            }
        }
        Ok(())
    }

    /// Update the flagged files depending of the input regex.
    pub fn input_regex(&mut self, char: char) -> Result<()> {
        self.menu.input.insert(char);
        self.flag_from_regex()?;
        Ok(())
    }

    /// Flag every file matching a typed regex.
    /// Move to the "first" found match
    pub fn flag_from_regex(&mut self) -> Result<()> {
        let input = self.menu.input.string();
        if input.is_empty() {
            return Ok(());
        }
        let paths = match self.current_tab().display_mode {
            Display::Directory => self.tabs[self.index].directory.paths(),
            Display::Tree => self.tabs[self.index].tree.paths(),
            _ => return Ok(()),
        };
        regex_flagger(&input, &paths, &mut self.menu.flagged)?;
        if !self.menu.flagged.is_empty() {
            self.tabs[self.index]
                .go_to_file(self.menu.flagged.selected().context("no selected file")?);
        }
        Ok(())
    }

    /// Open a the selected file with its opener
    pub fn open_selected_file(&mut self) -> Result<()> {
        let path = self.current_tab().current_file()?.path;
        self.open_single_file(&path)
    }

    /// Opens the selected file (single)
    pub fn open_single_file(&mut self, path: &Path) -> Result<()> {
        match self.internal_settings.opener.kind(path) {
            Some(Kind::Internal(Internal::NotSupported)) => self.mount_iso_drive(),
            Some(_) => self.internal_settings.open_single_file(path),
            None => Ok(()),
        }
    }

    /// Open every flagged file with their respective opener.
    pub fn open_flagged_files(&mut self) -> Result<()> {
        self.internal_settings
            .open_flagged_files(&self.menu.flagged)
    }

    fn ensure_iso_device_is_some(&mut self) -> Result<()> {
        if self.menu.iso_device.is_none() {
            let path = path_to_string(&self.current_tab().current_file()?.path);
            self.menu.iso_device = Some(IsoDevice::from_path(path));
        }
        Ok(())
    }

    /// Mount the currently selected file (which should be an .iso file) to
    /// `/run/media/$CURRENT_USER/fm_iso`
    /// Ask a sudo password first if needed. It should always be the case.
    fn mount_iso_drive(&mut self) -> Result<()> {
        if !self.menu.password_holder.has_sudo() {
            self.ask_password(Some(MountAction::MOUNT), PasswordUsage::ISO)?;
        } else {
            self.ensure_iso_device_is_some()?;
            let Some(ref mut iso_device) = self.menu.iso_device else {
                return Ok(());
            };
            if iso_device.mount(&current_username()?, &mut self.menu.password_holder)? {
                log_info!("iso mounter mounted {iso_device:?}");
                log_line!("iso : {iso_device}");
                let path = iso_device
                    .mountpoints
                    .clone()
                    .expect("mountpoint should be set");
                self.current_tab_mut().cd(Path::new(&path))?;
            };
            self.menu.iso_device = None;
        };

        Ok(())
    }

    /// Currently unused.
    /// Umount an iso device.
    pub fn umount_iso_drive(&mut self) -> Result<()> {
        if let Some(ref mut iso_device) = self.menu.iso_device {
            if !self.menu.password_holder.has_sudo() {
                self.ask_password(Some(MountAction::UMOUNT), PasswordUsage::ISO)?;
            } else {
                iso_device.umount(&current_username()?, &mut self.menu.password_holder)?;
            };
        }
        self.menu.iso_device = None;
        Ok(())
    }

    /// Mount an encrypted device.
    /// If sudo password isn't set, asks for it and returns,
    /// Else if device keypass isn't set, asks for it and returns,
    /// Else, mount the device.
    pub fn mount_encrypted_drive(&mut self) -> Result<()> {
        if !self.menu.password_holder.has_sudo() {
            self.ask_password(
                Some(MountAction::MOUNT),
                PasswordUsage::CRYPTSETUP(PasswordKind::SUDO),
            )
        } else if !self.menu.password_holder.has_cryptsetup() {
            self.ask_password(
                Some(MountAction::MOUNT),
                PasswordUsage::CRYPTSETUP(PasswordKind::CRYPTSETUP),
            )
        } else {
            let Some(Mountable::Encrypted(device)) = &self.menu.mount.selected() else {
                return Ok(());
            };
            if let Ok(true) = device.mount(&current_username()?, &mut self.menu.password_holder) {
                self.go_to_encrypted_drive(device.uuid.clone())?;
            }
            Ok(())
        }
    }

    /// Unmount the selected device.
    /// Will ask first for a sudo password which is immediatly forgotten.
    pub fn umount_encrypted_drive(&mut self) -> Result<()> {
        if !self.menu.password_holder.has_sudo() {
            self.ask_password(
                Some(MountAction::UMOUNT),
                PasswordUsage::CRYPTSETUP(PasswordKind::SUDO),
            )
        } else {
            let Some(Mountable::Encrypted(device)) = &self.menu.mount.selected() else {
                log_info!("Cannot find Encrypted device");
                return Ok(());
            };
            let success =
                device.umount_close_crypto(&current_username()?, &mut self.menu.password_holder)?;
            log_info!("umount_encrypted_drive: {success}");
            Ok(())
        }
    }
    /// Mount the selected encrypted device. Will ask first for sudo password and
    /// passphrase.
    /// Those passwords are always dropped immediatly after the commands are run.
    pub fn mount_normal_device(&mut self) -> Result<()> {
        let Some(device) = self.menu.mount.selected() else {
            return Ok(());
        };
        if device.is_mounted() {
            return Ok(());
        }
        if device.is_crypto() {
            return self.mount_encrypted_drive();
        }
        let Ok(success) = self.menu.mount.mount_selected_no_password() else {
            return Ok(());
        };
        if success {
            self.menu.mount.update(self.internal_settings.disks())?;
            self.go_to_normal_drive()?;
            return Ok(());
        }
        if !self.menu.password_holder.has_sudo() {
            self.ask_password(Some(MountAction::MOUNT), PasswordUsage::DEVICE)
        } else {
            if let Ok(true) = self
                .menu
                .mount
                .mount_selected(&mut self.menu.password_holder)
            {
                self.go_to_normal_drive()?;
            }
            Ok(())
        }
    }

    /// Move to the selected device mount point.
    pub fn go_to_normal_drive(&mut self) -> Result<()> {
        let Some(path) = self.menu.mount.selected_mount_point() else {
            return Ok(());
        };
        let tab = self.current_tab_mut();
        tab.cd(&path)?;
        tab.refresh_view()
    }

    /// Move to the mount point based on its onscreen index
    pub fn go_to_mount_per_index(&mut self, c: char) -> Result<()> {
        let Some(index) = c.to_digit(10) else {
            return Ok(());
        };
        self.menu.mount.set_index(index.saturating_sub(1) as _);
        self.go_to_normal_drive()
    }

    fn go_to_encrypted_drive(&mut self, uuid: Option<String>) -> Result<()> {
        self.menu.mount.update(&self.internal_settings.disks)?;
        let Some(mountpoint) = self.menu.mount.find_encrypted_by_uuid(uuid) else {
            return Ok(());
        };
        log_info!("mountpoint {mountpoint}");
        let tab = self.current_tab_mut();
        tab.cd(Path::new(&mountpoint))?;
        tab.refresh_view()
    }

    /// Unmount the selected device.
    /// Will ask first for a sudo password which is immediatly forgotten.
    pub fn umount_normal_device(&mut self) -> Result<()> {
        let Some(device) = self.menu.mount.selected() else {
            return Ok(());
        };
        if !device.is_mounted() {
            return Ok(());
        }
        if device.is_crypto() {
            return self.umount_encrypted_drive();
        }
        let Ok(success) = self.menu.mount.umount_selected_no_password() else {
            return Ok(());
        };
        if success {
            self.menu.mount.update(self.internal_settings.disks())?;
            return Ok(());
        }
        if !self.menu.password_holder.has_sudo() {
            self.ask_password(Some(MountAction::UMOUNT), PasswordUsage::DEVICE)
        } else {
            self.menu
                .mount
                .umount_selected(&mut self.menu.password_holder)
        }
    }

    /// Ejects a removable device (usb key, mtp etc.).
    pub fn eject_removable_device(&mut self) -> Result<()> {
        if self.menu.mount.is_empty() {
            return Ok(());
        }
        let success = self.menu.mount.eject_removable_device()?;
        if success {
            self.menu.mount.update(self.internal_settings.disks())?;
        }
        Ok(())
    }

    /// Reads and parse a shell command. Some arguments may be expanded.
    /// See [`crate::modes::shell_command_parser`] for more information.
    pub fn execute_shell_command_from_input(&mut self) -> Result<bool> {
        let shell_command = self.menu.input.string();
        self.execute_shell_command(shell_command, None, true)
    }

    /// Parse and execute a shell command and expand tokens like %s, %t etc.
    pub fn execute_shell_command(
        &mut self,
        shell_command: String,
        files: Option<Vec<String>>,
        capture_output: bool,
    ) -> Result<bool> {
        let command = append_files_to_shell_command(shell_command, files);
        let Ok(args) = shell_command_parser(&command, self) else {
            self.set_menu_mode(self.index, Menu::Nothing)?;
            return Ok(true);
        };
        self.execute_parsed_command(args, command, capture_output)
    }

    fn execute_parsed_command(
        &mut self,
        mut args: Vec<String>,
        shell_command: String,
        capture_output: bool,
    ) -> Result<bool> {
        let executable = args.remove(0);
        if is_sudo_command(&executable) {
            self.enter_sudo_mode(shell_command)?;
            return Ok(false);
        }
        let params: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        if executable == *SAME_WINDOW_TOKEN {
            self.internal_settings.open_in_window(&params)?;
            return Ok(true);
        }
        if !is_in_path(&executable) {
            log_line!("{executable} isn't in path.");
            return Ok(true);
        }
        if capture_output {
            match execute_and_capture_output(executable, &params) {
                Ok(output) => self.preview_command_output(output, shell_command),
                Err(e) => {
                    log_info!("Error {e:?}");
                    log_line!("Command {shell_command} disn't finish properly");
                }
            }
            Ok(true)
        } else {
            let _ = execute_without_output(executable, &params);
            Ok(true)
        }
    }

    fn enter_sudo_mode(&mut self, shell_command: String) -> Result<()> {
        self.menu.sudo_command = Some(shell_command);
        self.ask_password(None, PasswordUsage::SUDOCOMMAND)?;
        Ok(())
    }

    /// Ask for a password of some kind (sudo or device passphrase).
    fn ask_password(
        &mut self,
        encrypted_action: Option<MountAction>,
        password_dest: PasswordUsage,
    ) -> Result<()> {
        log_info!("ask_password");
        self.set_menu_mode(
            self.index,
            Menu::InputSimple(InputSimple::Password(encrypted_action, password_dest)),
        )
    }

    /// Attach the typed password to the correct receiver and
    /// execute the command requiring a password.
    pub fn execute_password_command(
        &mut self,
        action: Option<MountAction>,
        dest: PasswordUsage,
    ) -> Result<()> {
        let password = self.menu.input.string();
        self.menu.input.reset();
        if matches!(dest, PasswordUsage::CRYPTSETUP(PasswordKind::CRYPTSETUP)) {
            self.menu.password_holder.set_cryptsetup(password)
        } else {
            self.menu.password_holder.set_sudo(password)
        };
        let sudo_command = self.menu.sudo_command.to_owned();
        self.reset_menu_mode()?;
        self.dispatch_password(action, dest, sudo_command)?;
        Ok(())
    }

    /// Execute a new mark, saving it to a config file for futher use.
    pub fn marks_new(&mut self, c: char) -> Result<()> {
        let path = self.current_tab_mut().directory.path.clone();
        self.menu.marks.new_mark(c, &path)?;
        self.current_tab_mut().refresh_view()?;
        self.reset_menu_mode()?;
        self.refresh_status()
    }

    /// Execute a jump to a mark, moving to a valid path.
    /// If the saved path is invalid, it does nothing but reset the view.
    pub fn marks_jump_char(&mut self, c: char) -> Result<()> {
        if let Some(path) = self.menu.marks.get(c) {
            self.current_tab_mut().cd(&path)?;
        }
        self.current_tab_mut().refresh_view()?;
        self.reset_menu_mode()?;
        self.refresh_status()
    }

    /// Execute a new temp mark, saving it temporary for futher use.
    pub fn temp_marks_new(&mut self, c: char) -> Result<()> {
        let Some(index) = c.to_digit(10) else {
            return Ok(());
        };
        let path = self.current_tab_mut().directory.path.to_path_buf();
        self.menu.temp_marks.set_mark(index as _, path);
        self.current_tab_mut().refresh_view()?;
        self.reset_menu_mode()?;
        self.refresh_status()
    }

    /// Erase the current mark
    pub fn temp_marks_erase(&mut self) -> Result<()> {
        self.menu.temp_marks.erase_current_mark();
        Ok(())
    }

    /// Execute a jump to a temporay mark, moving to a valid path.
    /// If the saved path is invalid, it does nothing but reset the view.
    pub fn temp_marks_jump_char(&mut self, c: char) -> Result<()> {
        let Some(index) = c.to_digit(10) else {
            return Ok(());
        };
        if let Some(path) = self.menu.temp_marks.get_mark(index as _) {
            self.tabs[self.index].cd(path)?;
        }
        self.current_tab_mut().refresh_view()?;
        self.reset_menu_mode()?;
        self.refresh_status()
    }

    /// Recursively delete all flagged files.
    /// If we try to delete the current root of the tab, nothing is deleted but a warning is displayed.
    pub fn confirm_delete_files(&mut self) -> Result<()> {
        if self.menu.flagged.contains(self.current_tab().root_path()) {
            log_info!("Can't delete current root path");
            log_line!("Can't delete current root path");
            return Ok(());
        }
        self.menu.delete_flagged_files()?;
        self.reset_menu_mode()?;
        self.clear_flags_and_reset_view()?;
        self.refresh_status()
    }

    /// Empty the trash folder permanently.
    pub fn confirm_trash_empty(&mut self) -> Result<()> {
        self.menu.trash.empty_trash()?;
        self.reset_menu_mode()?;
        self.clear_flags_and_reset_view()?;
        Ok(())
    }

    /// Ask the new filenames and set the confirmation mode.
    pub fn bulk_ask_filenames(&mut self) -> Result<()> {
        let flagged = self.flagged_in_current_dir();
        let current_path = self.current_tab_path_str();
        self.menu.bulk.ask_filenames(flagged, &current_path)?;
        if let Some(temp_file) = self.menu.bulk.temp_file() {
            self.open_single_file(&temp_file)?;
            if self.internal_settings.opener.extension_use_term("txt") {
                self.fm_sender.send(FmEvents::BulkExecute)?;
            } else {
                self.menu.bulk.watch_in_thread(self.fm_sender.clone())?;
            }
        }
        Ok(())
    }

    /// Execute a bulk create / rename
    pub fn bulk_execute(&mut self) -> Result<()> {
        self.menu.bulk.get_new_names()?;
        self.set_menu_mode(
            self.index,
            Menu::NeedConfirmation(NeedConfirmation::BulkAction),
        )?;
        Ok(())
    }

    /// Execute the bulk action.
    pub fn confirm_bulk_action(&mut self) -> Result<()> {
        if let (Some(paths), Some(create)) = self.menu.bulk.execute()? {
            self.menu.flagged.update(paths);
            self.menu.flagged.extend(create);
        } else {
            self.menu.flagged.clear();
        };
        self.reset_menu_mode()?;
        self.reset_tabs_view()?;
        Ok(())
    }

    fn run_sudo_command(&mut self, sudo_command: Option<String>) -> Result<()> {
        let Some(sudo_command) = sudo_command else {
            log_info!("No sudo_command received from args.");
            return self.menu.clear_sudo_attributes();
        };
        self.set_menu_mode(self.index, Menu::Nothing)?;
        reset_sudo_faillock()?;
        let Some(command) = sudo_command.strip_prefix("sudo ") else {
            log_info!("run_sudo_command cannot run {sudo_command}. It doesn't start with 'sudo '");
            return self.menu.clear_sudo_attributes();
        };
        let args = shell_command_parser(command, self)?;
        if args.is_empty() {
            return self.menu.clear_sudo_attributes();
        }
        let Some(password) = self.menu.password_holder.sudo() else {
            log_info!("run_sudo_command password isn't set");
            return self.menu.clear_sudo_attributes();
        };
        let directory_of_selected = self.current_tab().directory_of_selected()?;
        let (success, stdout, stderr) =
            execute_sudo_command_with_password(&args, &password, directory_of_selected)?;
        log_info!("sudo command execution. success: {success}");
        self.menu.clear_sudo_attributes()?;
        if !success {
            log_line!("sudo command failed: {stderr}");
        }
        self.preview_command_output(stdout, sudo_command.to_owned());
        Ok(())
    }

    /// Dispatch the known password depending of which component set
    /// the `PasswordUsage`.
    #[rustfmt::skip]
    pub fn dispatch_password(
        &mut self,
        action: Option<MountAction>,
        dest: PasswordUsage,
        sudo_command: Option<String>,
    ) -> Result<()> {
        match (dest, action) {
            (PasswordUsage::ISO,            Some(MountAction::MOUNT))  => self.mount_iso_drive(),
            (PasswordUsage::ISO,            Some(MountAction::UMOUNT)) => self.umount_iso_drive(),
            (PasswordUsage::CRYPTSETUP(_),  Some(MountAction::MOUNT))  => self.mount_encrypted_drive(),
            (PasswordUsage::CRYPTSETUP(_),  Some(MountAction::UMOUNT)) => self.umount_encrypted_drive(),
            (PasswordUsage::DEVICE,         Some(MountAction::MOUNT))  => self.mount_normal_device(),
            (PasswordUsage::DEVICE,         Some(MountAction::UMOUNT)) => self.umount_normal_device(),
            (PasswordUsage::SUDOCOMMAND,    _)                         => self.run_sudo_command(sudo_command),
            (_,                             _)                         => Ok(()),
        }
    }

    /// Set the display to preview a command output
    pub fn preview_command_output(&mut self, output: String, command: String) {
        log_info!("preview_command_output for {command}:\n{output}");
        if output.is_empty() {
            return;
        }
        let _ = self.reset_menu_mode();
        self.current_tab_mut().set_display_mode(Display::Preview);
        let preview = PreviewBuilder::cli_info(&output, command);
        if let Preview::Text(text) = &preview {
            log_info!("preview is Text with: {text:?}");
        } else {
            log_info!("preview is empty ? {empty}", empty = preview.is_empty());
        }
        self.current_tab_mut().window.reset(preview.len());
        self.current_tab_mut().preview = preview;
    }

    /// Set the nvim listen address from what the user typed.
    pub fn update_nvim_listen_address(&mut self) {
        self.internal_settings.update_nvim_listen_address()
    }

    /// Execute a command requiring a confirmation (Delete, Move or Copy).
    /// The action is only executed if the user typed the char `y`
    pub fn confirm(&mut self, c: char, confirmed_action: NeedConfirmation) -> Result<()> {
        if c == 'y' {
            if let Ok(must_leave) = self.match_confirmed_mode(confirmed_action) {
                if must_leave {
                    return Ok(());
                }
            }
        }
        self.reset_menu_mode()?;
        self.current_tab_mut().refresh_view()?;

        Ok(())
    }

    /// Execute a `NeedConfirmation` action (delete, move, copy, empty trash)
    fn match_confirmed_mode(&mut self, confirmed_action: NeedConfirmation) -> Result<bool> {
        match confirmed_action {
            NeedConfirmation::Delete => self.confirm_delete_files(),
            NeedConfirmation::Move => self.cut_or_copy_flagged_files(CopyMove::Move),
            NeedConfirmation::Copy => self.cut_or_copy_flagged_files(CopyMove::Copy),
            NeedConfirmation::EmptyTrash => self.confirm_trash_empty(),
            NeedConfirmation::BulkAction => self.confirm_bulk_action(),
            NeedConfirmation::DeleteCloud => {
                self.cloud_confirm_delete()?;
                return Ok(true);
            }
        }?;
        Ok(false)
    }

    /// Execute an action when the header line was clicked.
    pub fn header_action(&mut self, col: u16, binds: &Bindings) -> Result<()> {
        if self.current_tab().display_mode.is_preview() {
            return Ok(());
        }
        Header::new(self, self.current_tab())?
            .action(col, !self.focus.is_left())
            .matcher(self, binds)
    }

    /// Execute an action when the footer line was clicked.
    pub fn footer_action(&mut self, col: u16, binds: &Bindings) -> Result<()> {
        log_info!("footer clicked col {col}");
        let is_right = self.index == 1;
        let action = match self.current_tab().display_mode {
            Display::Preview => return Ok(()),
            Display::Tree | Display::Directory => {
                let footer = Footer::new(self, self.current_tab())?;
                footer.action(col, is_right).to_owned()
            }
            Display::Fuzzy => return Ok(()),
        };
        log_info!("action: {action}");
        action.matcher(self, binds)
    }

    /// Change permission of the flagged files.
    /// Once the user has typed an octal permission like 754, it's applied to
    /// the file.
    /// Nothing is done if the user typed nothing or an invalid permission like
    /// 955.
    pub fn chmod(&mut self) -> Result<()> {
        if self.menu.input.is_empty() || self.menu.flagged.is_empty() {
            return Ok(());
        }
        let input_permission = &self.menu.input.string();
        Permissions::set_permissions_of_flagged(input_permission, &self.menu.flagged)?;
        self.reset_tabs_view()
    }

    /// Enter the chmod mode where user can chmod a file.
    pub fn set_mode_chmod(&mut self) -> Result<()> {
        if self.current_tab_mut().directory.is_empty() {
            return Ok(());
        }
        if self.menu.flagged.is_empty() {
            self.toggle_flag_for_selected();
        }
        self.set_menu_mode(self.index, Menu::InputSimple(InputSimple::Chmod))?;
        self.menu.replace_input_by_permissions();
        Ok(())
    }

    /// Execute a custom event on the selected file
    pub fn run_custom_command(&mut self, string: &str) -> Result<()> {
        self.execute_shell_command(string.to_owned(), None, true)?;
        Ok(())
    }

    /// Compress the flagged files into an archive.
    /// Compression method is chosen by the user.
    /// The archive is created in the current directory and is named "archive.tar.??" or "archive.zip".
    /// Files which are above the CWD are filtered out since they can't be added to an archive.
    /// Archive creation depends on CWD so we ensure it's set to the selected tab.
    pub fn compress(&mut self) -> Result<()> {
        let here = &self.current_tab().directory.path;
        set_current_dir(here)?;
        let files_with_relative_paths = self.flagged_or_selected_relative_to(here);
        if files_with_relative_paths.is_empty() {
            return Ok(());
        }
        match self
            .menu
            .compression
            .compress(files_with_relative_paths, here)
        {
            Ok(()) => (),
            Err(error) => log_info!("Error compressing files. Error: {error}"),
        }
        Ok(())
    }

    /// Sort all file in a tab with a sort key which is selected according to which char was pressed.
    pub fn sort_by_char(&mut self, c: char) -> Result<()> {
        self.current_tab_mut().sort(c)?;
        self.menu.reset();
        self.set_height_for_menu_mode(self.index, Menu::Nothing)?;
        self.tabs[self.index].menu_mode = Menu::Nothing;
        let len = self.menu.len(Menu::Nothing);
        let height = self.second_window_height()?;
        self.menu.window = ContentWindow::new(len, height);
        self.focus = self.focus.to_parent();
        Ok(())
    }

    /// The width of a displayed canvas.
    pub fn canvas_width(&self) -> Result<u16> {
        let full_width = self.term_width();
        if self.session.dual() && full_width >= MIN_WIDTH_FOR_DUAL_PANE {
            Ok(full_width / 2)
        } else {
            Ok(full_width)
        }
    }

    /// Executes a search in current folder, selecting the first file matching
    /// the current completion proposition.
    /// ie. If you typed `"jpg"` before, it will move to the first file
    /// whose filename contains `"jpg"`.
    /// The current order of files is used.
    fn search(&mut self) -> Result<()> {
        let Some(search) = self.build_search_from_input() else {
            self.current_tab_mut().search = Search::empty();
            return Ok(());
        };
        self.search_and_update(search)
    }

    fn build_search_from_input(&self) -> Option<Search> {
        let searched = &self.menu.input.string();
        if searched.is_empty() {
            return None;
        }
        Search::new(searched).ok()
    }

    fn search_and_update(&mut self, mut search: Search) -> Result<()> {
        search.execute_search(self.current_tab_mut())?;
        self.current_tab_mut().search = search;
        self.update_second_pane_for_preview()
    }

    fn search_again(&mut self) -> Result<()> {
        let search = self.current_tab().search.clone_with_regex();
        self.search_and_update(search)
    }

    /// Set a new filter.
    /// Doesn't reset the input.
    pub fn filter(&mut self) -> Result<()> {
        let filter = FilterKind::from_input(&self.menu.input.string());
        self.current_tab_mut().set_filter(filter)?;
        self.search_again()
    }

    /// input the typed char and update the filterkind.
    pub fn input_filter(&mut self, c: char) -> Result<()> {
        self.menu.input_insert(c)?;
        self.filter()
    }

    /// Open the picker menu. Does nothing if already in a picker.
    pub fn open_picker(&mut self) -> Result<()> {
        if self.current_tab().menu_mode.is_picker() || self.menu.input_history.filtered_is_empty() {
            return Ok(());
        }
        let menu = self.current_tab().menu_mode;
        let content = self.menu.input_history.filtered_as_list();
        self.menu.picker.set(
            Some(PickerCaller::Menu(menu)),
            menu.name_for_picker(),
            content,
        );

        self.set_menu_mode(self.index, Menu::Navigate(Navigate::Picker))
    }

    /// Load the selected cloud configuration file from the config folder and open navigation the menu.
    pub fn cloud_load_config(&mut self) -> Result<()> {
        let Some(picked) = self.menu.picker.selected() else {
            log_info!("nothing selected");
            return Ok(());
        };
        let Ok(cloud) = google_drive(picked) else {
            log_line!("Invalid config file {picked}");
            return Ok(());
        };
        self.menu.cloud = cloud;
        self.set_menu_mode(self.index, Menu::Navigate(Navigate::Cloud))
    }

    /// Open the cloud menu.
    /// If no cloud has been selected yet, all cloud config file will be displayed.
    /// if a cloud has been selected, it will open it.
    pub fn cloud_open(&mut self) -> Result<()> {
        if self.menu.cloud.is_set() {
            self.set_menu_mode(self.index, Menu::Navigate(Navigate::Cloud))
        } else {
            self.cloud_picker()
        }
    }

    fn cloud_picker(&mut self) -> Result<()> {
        let content = get_cloud_token_names()?;
        self.menu.picker.set(
            Some(PickerCaller::Cloud),
            Some("Pick a cloud provider".to_owned()),
            content,
        );
        self.set_menu_mode(self.index, Menu::Navigate(Navigate::Picker))
    }

    /// Disconnect from the current cloud and open the picker
    pub fn cloud_disconnect(&mut self) -> Result<()> {
        self.menu.cloud.disconnect();
        self.cloud_open()
    }

    /// Enter the delete mode and ask confirmation.
    /// Only the currently selected file can be deleted.
    pub fn cloud_enter_delete_mode(&mut self) -> Result<()> {
        self.set_menu_mode(
            self.index,
            Menu::NeedConfirmation(NeedConfirmation::DeleteCloud),
        )
    }

    /// Delete the selected file once a confirmation has been received from the user.
    pub fn cloud_confirm_delete(&mut self) -> Result<()> {
        self.menu.cloud.delete()?;
        self.set_menu_mode(self.index, Menu::Navigate(Navigate::Cloud))?;
        self.menu.cloud.refresh_current()?;
        self.menu.window.scroll_to(self.menu.cloud.index);
        Ok(())
    }

    /// Update the metadata of the current file.
    pub fn cloud_update_metadata(&mut self) -> Result<()> {
        self.menu.cloud.update_metadata()
    }

    /// Ask the user to enter a name for the new directory.
    pub fn cloud_enter_newdir_mode(&mut self) -> Result<()> {
        self.set_menu_mode(self.index, Menu::InputSimple(InputSimple::CloudNewdir))?;
        self.refresh_view()
    }

    /// Create the new directory in current path with the name the user entered.
    pub fn cloud_create_newdir(&mut self, dirname: String) -> Result<()> {
        self.menu.cloud.create_newdir(dirname)?;
        self.menu.cloud.refresh_current()
    }

    fn get_normal_selected_file(&self) -> Option<FileInfo> {
        let local_file = self.tabs[self.index].current_file().ok()?;
        match local_file.file_kind {
            FileKind::NormalFile => Some(local_file),
            _ => None,
        }
    }

    /// Upload the current file (tree or directory mode) the the current remote path.
    pub fn cloud_upload_selected_file(&mut self) -> Result<()> {
        let Some(local_file) = self.get_normal_selected_file() else {
            log_line!("Can only upload normal files.");
            return Ok(());
        };
        self.menu.cloud.upload(&local_file)?;
        self.menu.cloud.refresh_current()
    }

    /// Enter a file (download it) or the directory (explore it).
    pub fn cloud_enter_file_or_dir(&mut self) -> Result<()> {
        if let Some(entry) = self.menu.cloud.selected() {
            match entry.metadata().mode() {
                EntryMode::Unknown => (),
                EntryMode::FILE => self
                    .menu
                    .cloud
                    .download(self.current_tab().directory_of_selected()?)?,
                EntryMode::DIR => {
                    self.menu.cloud.enter_selected()?;
                    self.cloud_set_content_window_len()?;
                }
            };
        };
        Ok(())
    }

    fn cloud_set_content_window_len(&mut self) -> Result<()> {
        let len = self.menu.cloud.content.len();
        let height = self.second_window_height()?;
        self.menu.window = ContentWindow::new(len, height);
        Ok(())
    }

    /// Move to the parent folder if possible.
    /// Nothing is done in the root folder.
    pub fn cloud_move_to_parent(&mut self) -> Result<()> {
        self.menu.cloud.move_to_parent()?;
        self.cloud_set_content_window_len()?;
        Ok(())
    }

    pub fn toggle_flag_visual(&mut self) {
        if self.current_tab().visual {
            let Ok(file) = self.current_tab().current_file() else {
                return;
            };
            self.menu.flagged.toggle(&file.path)
        }
    }

    /// Parse and execute the received IPC message.
    /// IPC message can currently be of 3 forms:
    /// GO <path> -> cd to this path and select the file.
    /// KEY <key> -> act as if the key was pressed. <key> should be formated like in the config file.
    /// ACTION <action> -> execute the action. Similar to :<action><Enter>. <action> should be formated like in the config file.
    /// Other messages are ignored.
    /// Failed messages (path don't exists, wrongly formated key or action) are ignored silently.
    pub fn parse_ipc(&mut self, msg: String) -> Result<()> {
        let mut split = msg.split_whitespace();
        match split.next() {
            Some("GO") => {
                log_info!("Received IPC command GO");
                if let Some(dest) = split.next() {
                    log_info!("Received IPC command GO to {dest}");
                    let dest = tilde(dest);
                    self.current_tab_mut().cd_to_file(dest.as_ref())?
                }
            }
            Some("KEY") => {
                log_info!("Received IPC command KEY");
                if let Some(keyname) = split.next() {
                    if let Some(key) = from_keyname(keyname) {
                        self.fm_sender.send(FmEvents::Term(Event::Key(key)))?;
                        log_info!("Sent key event: {key:?}");
                    }
                }
            }
            Some("ACTION") => {
                log_info!("Received IPC command ACTION");
                if let Some(action_str) = split.next() {
                    if let Ok(action) = ActionMap::from_str(action_str) {
                        log_info!("Sent action event: {action:?}");
                        self.fm_sender.send(FmEvents::Action(action))?;
                    }
                }
            }
            Some(_unknown) => log_info!("Received unknown IPC command: {msg}"),
            None => (),
        };
        Ok(())
    }
}

fn find_keybind_from_fuzzy(line: &str) -> Result<KeyEvent> {
    let Some(keybind) = line.split(':').next() else {
        bail!("No keybind found");
    };
    let Some(key) = from_keyname(keybind.trim()) else {
        bail!("{keybind} isn't a valid Key name.");
    };
    Ok(key)
}
