use crate::app::Status;
use crate::app::Tab;
use crate::common::is_program_in_path;
use crate::common::DEFAULT_DRAGNDROP;
use crate::common::DIFF;
use crate::common::MEDIAINFO;
use crate::common::NITROGEN;
use crate::io::execute;
use crate::log_info;
use crate::log_line;
use crate::modes::Display;
use crate::modes::ExtensionKind;
use crate::modes::Preview;
use crate::modes::SelectableContent;

/// Bunch of commands which check :
/// 1. if a program is accessible from path
/// 2. find the selected file or group of files
/// 3. run the program with those arguments.
/// 4. eventually set the status & tab to display the output
///
/// Those commands must
/// 1. be infaillible,
/// 2. take only a single reference to `Status` or `Tab`,
/// 3. returns `(())`
/// If they can't get the expected result, they must return early, doing nothing.
pub struct SpecificCommand {}

impl SpecificCommand {
    /// Display mediainfo details of an image
    pub fn mediainfo(tab: &mut Tab) {
        if !matches!(tab.display_mode, Display::Tree | Display::Normal) {
            return;
        }
        if !is_program_in_path(MEDIAINFO) {
            log_line!("{} isn't installed", MEDIAINFO);
            return;
        }
        let Ok(file_info) = tab.current_file() else {
            return;
        };
        log_info!("selected {:?}", file_info);
        let Ok(preview) = Preview::mediainfo(&file_info.path) else {
            return;
        };
        tab.preview = preview;
        tab.window.reset(tab.preview.len());
        tab.set_display_mode(Display::Preview);
    }

    /// Display a diff between the first 2 flagged files or dir.
    pub fn diff(status: &mut Status) {
        if !matches!(
            status.current_tab().display_mode,
            Display::Tree | Display::Normal
        ) {
            return;
        }
        if status.menu.flagged.len() < 2 {
            return;
        };
        if !is_program_in_path(DIFF) {
            log_line!("{DIFF} isn't installed");
            return;
        }
        let Some(first_path) = &status.menu.flagged.content[0].to_str() else {
            return;
        };
        let Some(second_path) = &status.menu.flagged.content[1].to_str() else {
            return;
        };
        let Ok(preview) = Preview::diff(first_path, second_path) else {
            return;
        };
        let tab = status.current_tab_mut();
        tab.preview = preview;
        tab.window.reset(tab.preview.len());
        tab.set_display_mode(Display::Preview);
    }

    /// Set the current selected file as wallpaper with `nitrogen`.
    /// Requires `nitrogen` to be installed.
    pub fn set_wallpaper(tab: &Tab) {
        if !is_program_in_path(NITROGEN) {
            log_line!("nitrogen must be installed");
            return;
        }
        let Ok(fileinfo) = tab.current_file() else {
            return;
        };
        if !ExtensionKind::matcher(&fileinfo.extension).is(ExtensionKind::Image) {
            return;
        }
        let Ok(path_str) = tab.current_file_string() else {
            return;
        };
        let _ = execute(NITROGEN, &["--set-zoom-fill", "--save", &path_str]);
    }

    /// Executes a `dragon-drop` command on the selected file.
    /// It obviously requires the `dragon-drop` command to be installed.
    pub fn drag_n_drop(status: &mut Status) {
        if !is_program_in_path(DEFAULT_DRAGNDROP) {
            log_line!("{DEFAULT_DRAGNDROP} must be installed.");
            return;
        }
        let Ok(path_str) = status.current_tab().current_file_string() else {
            return;
        };

        let _ = execute(DEFAULT_DRAGNDROP, &[&path_str]);
    }
}
