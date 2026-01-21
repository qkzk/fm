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
use serde_yaml_ng::{from_reader, Value};
use strum::{EnumIter, IntoEnumIterator};
use syntect::{
    dumps::{from_binary, from_dump_file},
    highlighting::{Theme, ThemeSet},
};

use crate::{
    app::{build_plugins, PreviewerPlugin},
    common::CONFIG_FOLDER,
    common::{tilde, CONFIG_PATH, SYNTECT_THEMES_PATH},
    config::{
        read_normal_file_colorer, FileStyle, Gradient, MenuStyle, NormalFileColorer,
        PreferedImager, SyntectTheme, MAX_GRADIENT_NORMAL,
    },
    log_info,
};

/// Starting folder of the application. Read from arguments if any `-P ~/Downloads` else it uses the current folder: `.`.
pub static START_FOLDER: OnceLock<PathBuf> = OnceLock::new();

/// Store true if logging is enabled else false.
/// Set by the application itself and read before updating zoxide database.
pub static IS_LOGGING: OnceLock<bool> = OnceLock::new();

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
static SYNTECT_THEME: OnceLock<Theme> = OnceLock::new();

static PLUGINS: OnceLock<Vec<(String, PreviewerPlugin)>> = OnceLock::new();

static PREFERED_IMAGER: OnceLock<PreferedImager> = OnceLock::new();

pub fn get_prefered_imager() -> Option<&'static PreferedImager> {
    PREFERED_IMAGER.get()
}

/// Attach a map of name -> path to the `PLUGINS` static variable.
pub fn set_previewer_plugins(plugins: Vec<(String, String)>) -> Result<()> {
    let _ = PLUGINS.set(build_plugins(plugins));
    Ok(())
}

/// `PLUGINS` static map. Returns a map of name -> path.
pub fn get_previewer_plugins() -> Option<&'static Vec<(String, PreviewerPlugin)>> {
    PLUGINS.get()
}

/// Reads the syntect_theme configuration value and tries to load if from configuration files.
///
/// If it doesn't work, it will load the default set from binary file itself: monokai.
pub fn set_syntect_theme() -> Result<()> {
    let config_theme = SyntectTheme::from_config(CONFIG_PATH)?;
    if !set_syntect_theme_from_config(&config_theme.name) {
        set_syntect_theme_from_source_code()
    }
    Ok(())
}

pub fn set_prefered_imager() -> Result<()> {
    let prefered_imager = PreferedImager::from_config(CONFIG_PATH)?;
    let _ = PREFERED_IMAGER.set(prefered_imager);
    Ok(())
}

#[derive(EnumIter, Debug)]
enum SyntectThemeKind {
    TmTheme,
    Dump,
}

impl SyntectThemeKind {
    fn extension(&self) -> &str {
        match self {
            Self::TmTheme => "tmTheme",
            Self::Dump => "themedump",
        }
    }

    fn load(&self, themepath: &Path) -> Result<Theme> {
        match self {
            Self::TmTheme => ThemeSet::get_theme(themepath)
                .map_err(|e| anyhow!("Couldn't load syntect theme {e:}")),
            Self::Dump => {
                from_dump_file(themepath).map_err(|e| anyhow!("Couldn't load syntect theme {e:}"))
            }
        }
    }
}

fn set_syntect_theme_from_config(syntect_theme: &str) -> bool {
    let syntect_theme_path = PathBuf::from(tilde(SYNTECT_THEMES_PATH).as_ref());
    for kind in SyntectThemeKind::iter() {
        if load_syntect(&syntect_theme_path, syntect_theme, &kind) {
            return true;
        }
        log_info!("Couldn't load {syntect_theme} {kind:?}");
    }
    false
}

fn load_syntect(syntect_theme_path: &Path, syntect_theme: &str, kind: &SyntectThemeKind) -> bool {
    let mut full_path = syntect_theme_path.to_path_buf();
    full_path.push(syntect_theme);
    full_path.set_extension(kind.extension());
    if !full_path.exists() {
        return false;
    }
    let Ok(theme) = kind.load(&full_path) else {
        crate::log_info!("Syntect couldn't load {fp}", fp = full_path.display());
        return false;
    };
    let name = theme.name.clone();
    if SYNTECT_THEME.set(theme).is_ok() {
        log_info!("SYNTECT_THEME set to {name:?}");
        true
    } else {
        crate::log_info!("SYNTECT_THEME was already set!");
        false
    }
}

fn set_syntect_theme_from_source_code() {
    let _ = SYNTECT_THEME.set(from_binary(include_bytes!(
        "../../assets/themes/monokai.themedump"
    )));
}

/// Reads the syntect theme from memory. It should never be `None`.
pub fn get_syntect_theme() -> Option<&'static Theme> {
    SYNTECT_THEME.get()
}

static ICON: OnceLock<bool> = OnceLock::new();
static ICON_WITH_METADATA: OnceLock<bool> = OnceLock::new();

/// Does the user wants nerdfont icons ? Default: false.
pub fn with_icon() -> bool {
    *ICON.get().unwrap_or(&false)
}

/// Does the user wants nerdfont icons even if metadata are shown ? Default: false.
pub fn with_icon_metadata() -> bool {
    *ICON_WITH_METADATA.get().unwrap_or(&false)
}

fn set_start_folder(start_folder: &str) -> Result<()> {
    START_FOLDER
        .set(std::fs::canonicalize(tilde(start_folder).as_ref()).unwrap_or_default())
        .map_err(|_| anyhow!("Start folder shouldn't be set"))?;
    Ok(())
}

fn set_file_styles(yaml: &Option<Value>) -> Result<()> {
    FILE_STYLES
        .set(FileStyle::from_config(yaml))
        .map_err(|_| anyhow!("File colors shouldn't be set"))?;
    Ok(())
}

fn set_menu_styles(yaml: &Option<Value>) -> Result<()> {
    MENU_STYLES
        .set(MenuStyle::default().update(yaml))
        .map_err(|_| anyhow!("Menu colors shouldn't be set"))?;
    Ok(())
}

fn set_normal_file_colorer(yaml: &Option<Value>) -> Result<()> {
    let (start_color, stop_color) = read_normal_file_colorer(yaml);
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
pub fn set_configurable_static(
    start_folder: &str,
    plugins: Vec<(String, String)>,
    theme: String,
) -> Result<()> {
    let theme_yaml = read_theme(theme);
    set_start_folder(start_folder)?;
    set_menu_styles(&theme_yaml)?;
    set_file_styles(&theme_yaml)?;
    set_normal_file_colorer(&theme_yaml)?;
    set_icon_icon_with_metadata()?;
    set_syntect_theme()?;
    set_prefered_imager()?;
    set_previewer_plugins(plugins)
}

fn read_theme(theme: String) -> Option<Value> {
    read_yaml_value(&build_theme_path(theme))
}

fn build_theme_path(theme: String) -> PathBuf {
    let config_folder = tilde(CONFIG_FOLDER);
    let mut theme_path = PathBuf::from(config_folder.as_ref());
    theme_path.push("themes");
    theme_path.push(theme);
    theme_path.set_extension("yml");
    theme_path
}

fn read_yaml_value(path: &Path) -> Option<Value> {
    let Ok(file) = File::open(path) else {
        return None;
    };
    let Ok(yaml) = from_reader::<File, Value>(file) else {
        return None;
    };
    Some(yaml)
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
