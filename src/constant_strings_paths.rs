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
