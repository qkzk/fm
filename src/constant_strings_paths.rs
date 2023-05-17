/// Configuration folder path
pub static CONFIG_FOLDER: &str = "~/.config/fm";
/// Configuration file path
pub static CONFIG_PATH: &str = "~/.config/fm/config.yaml";
/// Filepath of the opener config file
pub static OPENER_PATH: &str = "~/.config/fm/opener.yaml";
/// Filepath of the TUIS configuration file
pub static TUIS_PATH: &str = "~/.config/fm/tuis.yaml";
/// Filepath of the LOG configuration file
pub static LOG_CONFIG_PATH: &str = "~/.config/fm/logging_config.yaml";
/// Path to the action log file
pub static ACTION_LOG_PATH: &str = "~/.config/fm/log/action_logger.log";
/// Path to the trash folder files
pub static TRASH_FOLDER_FILES: &str = "~/.local/share/Trash/files";
/// Path to the trash folder info file
pub static TRASH_FOLDER_INFO: &str = "~/.local/share/Trash/info";
/// Log file path. Rotating file logs are created in the same directeroy
pub static LOG_PATH: &str = "~/.config/fm/fm{}";
/// File where marks are stored.
pub static MARKS_FILEPATH: &str = "~/.config/fm/marks.cfg";
/// Temporary folder used when bulkrenaming files
pub static TMP_FOLDER_PATH: &str = "/tmp";
/// Default terminal application used when openening a program in shell or starting a new shell
pub static DEFAULT_TERMINAL_APPLICATION: &str = "st";
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
/// Default program used to drag and drop files
pub static DEFAULT_DRAGNDROP: &str = "dragon-drop";
/// Array of text representation of a file permissions.
/// The index of each string gives a correct representation.
pub static PERMISSIONS_STR: [&str; 8] = ["---", "--x", "-w-", "-wx", "r--", "r-x", "rw-", "rwx"];
/// Description of the application.
pub static HELP_FIRST_SENTENCE: &str = "fm: a dired / ranger like file manager. ";
/// Description of the content below, aka the help itself.
pub static HELP_SECOND_SENTENCE: &str = "Keybindings";
/// Description of the content below, aka the action log file
pub static LOG_FIRST_SENTENCE: &str = "Logs: ";
/// Description of the content below, aka what is logged there.
pub static LOG_SECOND_SENTENCE: &str = "Last actions affecting the file tree";
/// Video thumbnails
pub static THUMBNAIL_PATH: &str = "/tmp/thumbnail.png";
/// Array of hardcoded shortcuts with standard *nix paths.
pub static HARDCODED_SHORTCUTS: [&str; 9] = [
    "/",
    "/dev",
    "/etc",
    "/mnt",
    "/opt",
    "/run/media",
    "/tmp",
    "/usr",
    "/var",
];
pub static BAT_EXECUTABLE: &str = "bat {} --color=always";
pub static CAT_EXECUTABLE: &str = "cat {}";
pub static RG_EXECUTABLE: &str = "rg --line-number \"{}\"";
pub static GREP_EXECUTABLE: &str = "grep -rI --line-number \"{}\"";
/// Sort presentation for the second window
pub static SORT_LINES: [&str; 7] = [
    "k:         by kind (default)",
    "n:         by name",
    "m:         by modification time",
    "s:         by size",
    "e:         by extension",
    "",
    "r:         reverse current sort",
];
/// Chmod presentation for the second window
pub static CHMOD_LINES: [&str; 5] = [
    "Type an octal mode like 754.",
    "",
    "4:      read",
    "2:      write",
    "1:      execute",
];
/// Filter presentation for the second window
pub static FILTER_LINES: [&str; 6] = [
    "Type the initial of the filter and an expression if needed",
    "",
    "n {name}:      by name",
    "e {extension}: by extension",
    "d:             only directories",
    "a:             reset",
];
/// Password input presentation for the second window
pub static PASSWORD_LINES: [&str; 1] =
    ["Type your password. It will be forgotten immediatly after use."];
/// Shell presentation for the second window
pub static SHELL_LINES: [&str; 11] = [
    "Type a shell command",
    "",
    "`sudo` commands are supported.",
    "Pipes, redirections ( | < > >> ) and shell specific syntax (*) aren't supported.",
    "",
    "Some expression can be expanded:",
    "%d:    current directory",
    "%e:    selected file extension",
    "%f:    flagged files",
    "%n:    selected filename",
    "%s:    selected filepath",
];
/// Nvim address setter presentation for second window
pub static NVIM_ADDRESS_LINES: [&str; 4] = [
    "Type the Neovim RPC address.",
    "",
    "You can get it from Neovim with :",
    "`:echo v:servername`",
];
/// Regex matcher presentation for second window
pub static REGEX_LINES: [&str; 3] = [
    "Type a regular expression",
    "",
    "Flag every file in current directory matching the typed regex",
];
/// Newdir presentation for second window
pub static NEWDIR_LINES: [&str; 3] = [
    "mkdir a new directory",
    "",
    "Nothing is done if the directory already exists",
];
/// New file presentation for second window
pub static NEWFILE_LINES: [&str; 3] = [
    "touch a new file",
    "",
    "Nothing is done if the file already exists",
];
/// Rename presentation for second window
pub static RENAME_LINES: [&str; 3] = [
    "rename the selected file",
    "",
    "Nothing is done if the file already exists",
];
