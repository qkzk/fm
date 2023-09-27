/// Configuration folder path
pub const CONFIG_FOLDER: &str = "~/.config/fm";
/// Configuration file path
pub const CONFIG_PATH: &str = "~/.config/fm/config.yaml";
/// Filepath of the opener config file
pub const OPENER_PATH: &str = "~/.config/fm/opener.yaml";
/// Filepath of the TUIS configuration file
pub const TUIS_PATH: &str = "~/.config/fm/tuis.yaml";
/// Filepath of the LOG configuration file
pub const LOG_CONFIG_PATH: &str = "~/.config/fm/logging_config.yaml";
/// Path to the action log file
pub const ACTION_LOG_PATH: &str = "~/.config/fm/log/action_logger.log";
/// Path to the trash folder files
pub const TRASH_FOLDER_FILES: &str = "~/.local/share/Trash/files";
/// Path to the trash folder info file
pub const TRASH_FOLDER_INFO: &str = "~/.local/share/Trash/info";
/// Log file path. Rotating file logs are created in the same directeroy
pub const LOG_PATH: &str = "~/.config/fm/fm{}";
/// File where marks are stored.
pub const MARKS_FILEPATH: &str = "~/.config/fm/marks.cfg";
/// Temporary folder used when bulkrenaming files
pub const TMP_FOLDER_PATH: &str = "/tmp";
/// Default terminal application used when openening a program in shell or starting a new shell
pub const DEFAULT_TERMINAL_APPLICATION: &str = "st";
/// Opener used to play audio files. Does it require a terminal ?
pub const DEFAULT_AUDIO_OPENER: (&str, bool) = ("mocp", true);
/// Program used to to display images. Does it require a terminal ?
pub const DEFAULT_IMAGE_OPENER: (&str, bool) = ("viewnior", false);
/// Program used to open "office" documents (word, libreoffice etc). Does it require a terminal ?
pub const DEFAULT_OFFICE_OPENER: (&str, bool) = ("libreoffice", false);
/// Program used to open readable documents (pdf, ebooks). Does it require a terminal ?
pub const DEFAULT_READABLE_OPENER: (&str, bool) = ("zathura", false);
/// Program used to open text files. Does it require a terminal ?
pub const DEFAULT_TEXT_OPENER: (&str, bool) = ("nvim", true);
/// Program used to open unknown files. Does it require a terminal ?
pub const DEFAULT_OPENER: (&str, bool) = ("xdg-open", false);
/// Program used to open vectorial images. Does it require a terminal ?
pub const DEFAULT_VECTORIAL_OPENER: (&str, bool) = ("inkscape", false);
/// Program used to open videos. Does it require a terminal ?
pub const DEFAULT_VIDEO_OPENER: (&str, bool) = ("mpv", false);
/// Default program used to drag and drop files
pub const DEFAULT_DRAGNDROP: &str = "dragon-drop";
/// Array of text representation of a file permissions.
/// The index of each string gives a correct representation.
pub const PERMISSIONS_STR: [&str; 8] = ["---", "--x", "-w-", "-wx", "r--", "r-x", "rw-", "rwx"];
/// Description of the application.
pub const HELP_FIRST_SENTENCE: &str = "fm: a dired / ranger like file manager. ";
/// Description of the content below, aka the help itself.
pub const HELP_SECOND_SENTENCE: &str = "Keybindings";
/// Description of the content below, aka the action log file
pub const LOG_FIRST_SENTENCE: &str = "Logs: ";
/// Description of the content below, aka what is logged there.
pub const LOG_SECOND_SENTENCE: &str = "Last actions affecting the file tree";
/// Video thumbnails
pub const THUMBNAIL_PATH: &str = "/tmp/thumbnail.png";
/// Array of hardcoded shortcuts with standard *nix paths.
pub const HARDCODED_SHORTCUTS: [&str; 9] = [
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
pub const BAT_EXECUTABLE: &str = "bat {} --color=always";
pub const CAT_EXECUTABLE: &str = "cat {}";
pub const RG_EXECUTABLE: &str = "rg --line-number \"{}\"";
pub const GREP_EXECUTABLE: &str = "grep -rI --line-number \"{}\"";
pub const SSHFS_EXECUTABLE: &str = "sshfs";
pub const NOTIFY_EXECUTABLE: &str = "notity-send";
/// Sort presentation for the second window
pub const SORT_LINES: [&str; 7] = [
    "k:         by kind (default)",
    "n:         by name",
    "m:         by modification time",
    "s:         by size",
    "e:         by extension",
    "",
    "r:         reverse current sort",
];
pub const REMOTE_LINES: [&str; 4] = [
    "Mount a directory with sshfs",
    "Type the arguments as below, separated by a space",
    "",
    "username hostname remote_path",
];
/// Chmod presentation for the second window
pub const CHMOD_LINES: [&str; 5] = [
    "Type an octal mode like 754.",
    "",
    "4:      read",
    "2:      write",
    "1:      execute",
];
/// Filter presentation for the second window
pub const FILTER_LINES: [&str; 6] = [
    "Type the initial of the filter and an expression if needed",
    "",
    "n {name}:      by name",
    "e {extension}: by extension",
    "d:             only directories",
    "a:             reset",
];
/// Password input presentation for the second window
pub const PASSWORD_LINES: [&str; 1] =
    ["Type your password. It will be forgotten immediatly after use."];
/// Shell presentation for the second window
pub const SHELL_LINES: [&str; 11] = [
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
pub const NVIM_ADDRESS_LINES: [&str; 4] = [
    "Type the Neovim RPC address.",
    "",
    "You can get it from Neovim with :",
    "`:echo v:servername`",
];
/// Regex matcher presentation for second window
pub const REGEX_LINES: [&str; 3] = [
    "Type a regular expression",
    "",
    "Flag every file in current directory matching the typed regex",
];
/// Newdir presentation for second window
pub const NEWDIR_LINES: [&str; 3] = [
    "mkdir a new directory",
    "",
    "Nothing is done if the directory already exists",
];
/// New file presentation for second window
pub const NEWFILE_LINES: [&str; 3] = [
    "touch a new file",
    "",
    "Nothing is done if the file already exists",
];
/// Rename presentation for second window
pub const RENAME_LINES: [&str; 3] = [
    "rename the selected file",
    "",
    "Nothing is done if the file already exists",
];
/// Executable commands whose output is a text to be displayed in terminal
pub const CLI_INFO_COMMANDS: [&str; 4] = ["duf", "inxi -v 2 --color", "neofetch", "lsusb"];
/// Wallpaper executable
pub const NITROGEN: &str = "nitrogen";
