use std::{
    fs::File,
    ops::DerefMut,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::{anyhow, Result};
use nucleo::Matcher;
use parking_lot::{Mutex, MutexGuard};
use ratatui::style::Color;
use serde_yml::{from_reader, Value};
use syntect::highlighting::Theme;

use crate::common::{tilde, CONFIG_PATH};
use crate::config::{
    read_normal_file_colorer, FileStyle, Gradient, MenuStyle, NormalFileColorer,
    MAX_GRADIENT_NORMAL,
};

/// Starting folder of the application. Read from arguments if any `-P ~/Downloads` else it uses the current folder: `.`.
pub static START_FOLDER: OnceLock<PathBuf> = OnceLock::new();

/// Colors read from the config file.
/// We define a colors for every kind of file except normal files.
/// Colors for normal files are calculated from their extension and
/// are greens or blues.
///
/// Colors are setup on start and never change afterwards.
pub static FILE_STYLES: OnceLock<FileStyle> = OnceLock::new();

/// Menu color struct
pub static MENU_STYLES: OnceLock<MenuStyle> = OnceLock::new();

/// Defines a palette which will color the "normal" files based on their extension.
/// We try to read a yaml value and pick one of 3 palettes :
/// "green-red", "blue-green", "blue-red", "red-green", "red-blue", "green-blue" which is the default.
/// "custom" will create a gradient from start_palette to end_palette. Both values should be "rgb(u8, u8, u8)".
pub static COLORER: OnceLock<fn(usize) -> Color> = OnceLock::new();

/// Gradient for normal files
pub static ARRAY_GRADIENT: OnceLock<[Color; MAX_GRADIENT_NORMAL]> = OnceLock::new();

/// Highlighting theme color used to preview code file
pub static MONOKAI_THEME: OnceLock<Theme> = OnceLock::new();

/// Does the user wants nerdfont icons ? Default: false.
pub static ICON: OnceLock<bool> = OnceLock::new();
/// Does the user wants nerdfont icons even if metadata are shown ? Default: false.
pub static ICON_WITH_METADATA: OnceLock<bool> = OnceLock::new();

fn set_start_folder(start_folder: &str) -> Result<()> {
    START_FOLDER
        .set(std::fs::canonicalize(tilde(start_folder).as_ref()).unwrap_or_default())
        .map_err(|_| anyhow!("Start folder shouldn't be set"))?;
    Ok(())
}

fn set_file_styles() -> Result<()> {
    FILE_STYLES
        .set(FileStyle::from_config())
        .map_err(|_| anyhow!("File colors shouldn't be set"))?;
    Ok(())
}

fn set_menu_styles() -> Result<()> {
    MENU_STYLES
        .set(MenuStyle::default().update())
        .map_err(|_| anyhow!("Menu colors shouldn't be set"))?;
    Ok(())
}

fn set_normal_file_colorer() -> Result<()> {
    let (start_color, stop_color) = read_normal_file_colorer();
    ARRAY_GRADIENT
        .set(Gradient::new(start_color, stop_color, MAX_GRADIENT_NORMAL).as_array()?)
        .map_err(|_| anyhow!("Gradient shouldn't be set"))?;
    COLORER
        .set(NormalFileColorer::colorer as fn(usize) -> Color)
        .map_err(|_| anyhow!("Colorer shouldn't be set"))?;

    Ok(())
}

fn read_yaml_bool(yaml: &Value, key: &str) -> Option<bool> {
    yaml[key].as_bool()
}

fn read_icon_icon_with_metadata() -> (bool, bool) {
    let Ok(file) = File::open(Path::new(&tilde(CONFIG_PATH).to_string())) else {
        crate::log_info!("Couldn't read config file at {CONFIG_PATH}");
        return (false, false);
    };
    let Ok(yaml) = from_reader::<File, Value>(file) else {
        return (false, false);
    };
    let mut icon: bool = false;
    let mut icon_with_metadata: bool = false;
    if let Some(i) = read_yaml_bool(&yaml, "icon") {
        icon = i;
    }
    if !icon {
        icon_with_metadata = false;
    } else if let Some(icon_with) = read_yaml_bool(&yaml, "icon_with_metadata") {
        icon_with_metadata = icon_with;
    }
    (icon, icon_with_metadata)
}

/// Read `icon` and `icon_with_metadata` from the config file and store them in static values.
///
/// `icon_with_metadata` can't be true if `icon` is false, even if the user set it to true.
/// If the user hasn't installed nerdfont, the icons can't be shown properly and `icon` shouldn't be shown.
/// It leads to a quite complex parsing:
/// - If the file can't be read (should never happen, the application should have quit already), both icon & icon_with_metadata are false,
/// - If the values aren't in the yaml file, both are false,
/// - If icon is false, icon_with_metadata is false,
/// - Otherwise, we use the values from the file.
pub fn set_icon_icon_with_metadata() -> Result<()> {
    let (icon, icon_with_metadata) = read_icon_icon_with_metadata();
    ICON.set(icon)
        .map_err(|_| anyhow!("ICON shouldn't be set"))?;
    ICON_WITH_METADATA
        .set(icon_with_metadata)
        .map_err(|_| anyhow!("ICON_WITH_METADATA shouldn't be set"))?;
    Ok(())
}

/// Set all the values which could be configured from config file or arguments staticly.
/// It allows us to read those values globally without having to pass them through to every function.
/// All values use a [`std::sync::OnceLock`] internally.
pub fn set_configurable_static(start_folder: &str) -> Result<()> {
    set_start_folder(start_folder)?;
    set_menu_styles()?;
    set_file_styles()?;
    set_normal_file_colorer()?;
    set_icon_icon_with_metadata()
}

/// Copied from [Helix](https://github.com/helix-editor/helix/blob/master/helix-core/src/fuzzy.rs)
///
/// A mutex which is instancied lazylly.
/// The mutex is created with `None` as value and, once locked, is instancied if necessary.
pub struct LazyMutex<T> {
    inner: Mutex<Option<T>>,
    init: fn() -> T,
}

impl<T> LazyMutex<T> {
    /// Instanciate a new `LazyMutex` with `None` as value.
    pub const fn new(init: fn() -> T) -> Self {
        Self {
            inner: Mutex::new(None),
            init,
        }
    }

    /// Lock the mutex.
    /// At the first call, the value is created with the `init` function passed to `new`.
    /// Other calls won't have to do it. We just get the already created value.
    pub fn lock(&self) -> impl DerefMut<Target = T> + '_ {
        MutexGuard::map(self.inner.lock(), |val| val.get_or_insert_with(self.init))
    }
}

/// A nucleo matcher behind a lazy mutex.
/// Instanciated once and lazylly.
pub static MATCHER: LazyMutex<Matcher> = LazyMutex::new(Matcher::default);
