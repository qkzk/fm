/// Configuration file path
pub static CONFIG_PATH: &str = "~/.config/fm/config.yaml";
/// Default terminal application used when openening a program in shell or starting a new shell
pub static DEFAULT_TERMINAL_APPLICATION: &str = "st";
/// Log file path. Rotating file logs are created in the same directeroy
pub static LOG_PATH: &str = "~/.config/fm/fm{}";
/// File where marks are stored.
pub static MARKS_FILEPATH: &str = "~/.config/fm/marks.cfg";
/// Temporary folder used when bulkrenaming files
pub static TMP_FOLDER_PATH: &str = "/tmp";
/// Opener used to play audio files. Does it require a terminal ?
pub static DEFAULT_AUDIO_OPENER: (&str, bool) = ("mocp", true);
/// Program used to to display images. Does it require a terminal ?
pub static DEFAULT_IMAGE_OPENER: (&str, bool) = ("viewnior", false);
/// Program used to open "office" documents (word, libreoffice etc). Does it require a terminal ?
pub static DEFAULT_OFFICE_OPENER: (&str, bool) = ("libreoffice", false);
/// Program used to open readable documents (pdf, ebooks). Does it require a terminal ?
pub static DEFAULT_READABLE_OPENER: (&str, bool) = ("zathura", false);
/// Program used to open text files. Does it require a terminal ?
pub static DEFAULT_TEXT_OPENER: (&str, bool) = ("nvim", true);
/// Program used to open unknown files. Does it require a terminal ?
pub static DEFAULT_OPENER: (&str, bool) = ("xdg-open", false);
/// Program used to open vectorial images. Does it require a terminal ?
pub static DEFAULT_VECTORIAL_OPENER: (&str, bool) = ("inkscape", false);
/// Program used to open videos. Does it require a terminal ?
pub static DEFAULT_VIDEO_OPENER: (&str, bool) = ("mpv", false);
/// Filepath of the opener config file
pub static OPENER_PATH: &str = "~/.config/fm/opener.yaml";
/// Default program used to drag and drop files
pub static DEFAULT_DRAGNDROP: &str = "dragon-drop";
/// Array of text representation of a file permissions.
/// The index of each string gives a correct representation.
pub static PERMISSIONS_STR: [&str; 8] = ["---", "--x", "-w-", "-wx", "r--", "r-x", "rw-", "rwx"];
/// Description of the application.
pub static HELP_FIRST_SENTENCE: &str = "fm: a dired / ranger like file manager. ";
/// Description of the content below, aka the help itself.
pub static HELP_SECOND_SENTENCE: &str = "Keybindings";
/// nvim-send is a rust program which can send commands to neovim
pub static NVIM_RPC_SENDER: &str = "nvim-send";
/// Filter presentation for the second line
pub static FILTER_PRESENTATION: &str =
    "By name: n expr, by ext: e expr, only directories: d, reset: a";
pub static HARDCODED_SHORTCUTS: [&str; 9] = [
    "/",
    "/dev",
    "/etc",
    "/media",
    "/mnt",
    "/opt",
    "/run/media",
    "/tmp",
    "/usr",
];
/// Path to the trash folder files
pub static TRASH_FOLDER_FILES: &str = "~/.local/share/Trash/files";
/// Path to the trash folder info file
pub static TRASH_FOLDER_INFO: &str = "~/.local/share/Trash/info";
