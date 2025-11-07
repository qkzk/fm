/// Configuration folder path
pub const CONFIG_FOLDER: &str = "~/.config/fm";
/// Configuration file path
pub const CONFIG_PATH: &str = "~/.config/fm/config.yaml";
/// Session file path
pub const SESSION_PATH: &str = "~/.config/fm/session.yaml";
/// Filepath of the opener config file
pub const OPENER_PATH: &str = "~/.config/fm/opener.yaml";
/// Filepath of the TUIS configuration file
pub const TUIS_PATH: &str = "~/.config/fm/tuis.yaml";
/// Filepath of the CLI configuration file
pub const CLI_PATH: &str = "~/.config/fm/cli.yaml";
/// Inputhistory
pub const INPUT_HISTORY_PATH: &str = "~/.config/fm/log/input_history.log";
/// Syntect theme paths
pub const SYNTECT_THEMES_PATH: &str = "~/.config/fm/syntect_themes/";
/// Path to the normal log file
pub const NORMAL_LOG_PATH: &str = "~/.config/fm/log/fm.log";
/// Path to the action log file
pub const ACTION_LOG_PATH: &str = "~/.config/fm/log/action_logger.log";
/// Path to the trash folder files
pub const TRASH_FOLDER_FILES: &str = "~/.local/share/Trash/files";
/// Path to the trash folder info file
pub const TRASH_FOLDER_INFO: &str = "~/.local/share/Trash/info";
/// Trash info files extension. Watchout it includes the final '.'
pub const TRASH_INFO_EXTENSION: &str = ".trashinfo";
/// File where marks are stored.
pub const MARKS_FILEPATH: &str = "~/.config/fm/marks.cfg";
/// Temporary folder used when bulkrenaming files
pub const TMP_FOLDER_PATH: &str = "/tmp";
/// Video thumbnails
pub const TMP_THUMBNAILS_DIR: &str = "/tmp/fm-thumbnails";
/// Default syntect theme, theme is hardcoded into binary
pub const SYNTECT_DEFAULT_THEME: &str = "monokai";
/// setsid. Installed in most distros
pub const SETSID: &str = "setsid";
/// Opener used to play audio files. Does it require a terminal ?
pub const OPENER_AUDIO: (&str, bool) = ("mocp", true);
/// Program used to to display images. Does it require a terminal ?
pub const OPENER_IMAGE: (&str, bool) = ("viewnior", false);
/// Program used to open "office" documents (word, libreoffice etc). Does it require a terminal ?
pub const OPENER_OFFICE: (&str, bool) = ("libreoffice", false);
/// Program used to open readable documents (pdf, ebooks). Does it require a terminal ?
pub const OPENER_READABLE: (&str, bool) = ("zathura", false);
/// Program used to open text files. Does it require a terminal ?
pub const OPENER_TEXT: (&str, bool) = ("nvim", true);
/// Program used to open unknown files. Does it require a terminal ?
pub const OPENER_DEFAULT: (&str, bool) = ("xdg-open", false);
/// Program used to open vectorial images. Does it require a terminal ?
pub const OPENER_VECT: (&str, bool) = ("inkscape", false);
/// Program used to open videos. Does it require a terminal ?
pub const OPENER_VIDEO: (&str, bool) = ("mpv", false);
/// Array of text representation of a file permissions.
/// The index of each string gives a correct representation.
pub const NORMAL_PERMISSIONS_STR: [&str; 8] =
    ["---", "--x", "-w-", "-wx", "r--", "r-x", "rw-", "rwx"];
pub const SETUID_PERMISSIONS_STR: [&str; 8] =
    ["--S", "--s", "-wS", "-ws", "r-S", "r-s", "rwS", "rws"];
pub const SETGID_PERMISSIONS_STR: [&str; 8] = SETUID_PERMISSIONS_STR;
pub const STICKY_PERMISSIONS_STR: [&str; 8] =
    ["--T", "--t", "-w-", "-wt", "r-T", "r-t", "rwT", "rwt"];
/// Description of the application.
pub const HELP_FIRST_SENTENCE: &str = " fm: a dired / ranger like file manager. ";
/// Description of the content below, aka the help itself.
pub const HELP_SECOND_SENTENCE: &str = " Keybindings ";
/// Description of the content below, aka the action log file
pub const LOG_FIRST_SENTENCE: &str = " Logs: ";
/// Description of the content below, aka what is logged there.
pub const LOG_SECOND_SENTENCE: &str = " Last actions affecting the file tree";
/// Ueberzug image thumbnails
pub const THUMBNAIL_PATH_PNG: &str = "/tmp/fm_thumbnail.png";
/// Ueberzug image thumbnails
pub const THUMBNAIL_PATH_JPG: &str = "/tmp/fm_thumbnail.jpg";
/// Ueberzug image for videos, without extension
pub const THUMBNAIL_PATH_NO_EXT: &str = "/tmp/fm_thumbnail";
/// Libreoffice pdf output
pub const CALC_PDF_PATH: &str = "/tmp/fm_calc.pdf";
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
/// Ripgrep & its parameters
pub const RG_EXECUTABLE: &str = "rg --line-number --color=never .";
/// Grep and its parameters
pub const GREP_EXECUTABLE: &str = "grep -rI --line-number .";
/// sshfs executable
pub const SSHFS_EXECUTABLE: &str = "sshfs";
/// Notify app executable
pub const NOTIFY_EXECUTABLE: &str = "notity-send";
/// Eject (of removable device) executable
pub const EJECT_EXECUTABLE: &str = "eject";
/// Encrypted devices bind description
pub const ENCRYPTED_DEVICE_BINDS: &str = "m: mount   --   u: unmount   --   g: go to mount point";
/// Sort presentation for the second window
pub const SORT_LINES: [&str; 9] = [
    "Type the initial",
    "",
    "k:  by kind (default)",
    "n:  by name",
    "m:  by modification time",
    "s:  by size",
    "e:  by extension",
    "",
    "r:  reverse current sort",
];
/// Remote menu presentation
pub const REMOTE_LINES: [&str; 5] = [
    "Mount a directory with sshfs",
    "Type the arguments as below, separated by a space.",
    "Port and local path are optional",
    "",
    "username hostname:port remote_path local_path",
];
/// Presentation of new dir. creation in cloud
pub const CLOUD_NEWDIR_LINES: [&str; 1] = ["Create a new directory in current cloud path"];
/// Chmod presentation for the second window
pub const CHMOD_LINES: [&str; 5] = [
    "Type an octal like \"754\", a text like \"rwxr.xr..\" or \"a+x\"",
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
pub const PASSWORD_LINES_SUDO: [&str; 2] = [
    "Type your sudo password.",
    "It will be forgotten immediatly after use.",
];
/// Presentation of the passkey input
pub const PASSWORD_LINES_DEVICE: [&str; 2] = [
    "Type the device passkey.",
    "It will be forgotten immediatly after use.",
];
/// Shell presentation for the second window
pub const SHELL_LINES: [&str; 13] = [
    "Type a shell command",
    "",
    "`sudo` commands are supported.",
    "Pipes, redirections ( | < > >> ) and shell specific syntax (*) aren't supported.",
    "",
    "Some expression can be expanded:",
    "%s: the selected file",
    "%f: the flagged files",
    "%e: the extension of the file",
    "%n: the filename only",
    "%d: the full path of the current directory",
    "%t: execute the command in the same window",
    "%c: the current clipboard as a string",
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
/// Trash presentation
pub const TRASH_CONFIRM_LINE: &str =
    "Up, Down: navigation - Enter: restore the selected file - x: delete permanently - ";
/// Mediainfo (used to preview media files) executable
pub const MEDIAINFO: &str = "mediainfo";
/// ueberzug (used to preview images, videos & fonts)
pub const UEBERZUG: &str = "ueberzug";
/// fontimage (used to preview fonts)
pub const FONTIMAGE: &str = "fontimage";
/// ffmpeg (used to preview video thumbnail)
pub const FFMPEG: &str = "ffmpeg";
/// rsvg-convert (used to preview svg files)
pub const RSVG_CONVERT: &str = "rsvg-convert";
/// jupyter. used to preview notebooks (.ipynb)
pub const JUPYTER: &str = "jupyter";
/// pandoc. used to preview .doc & .odb documents
pub const PANDOC: &str = "pandoc";
/// isoinfo. used to preview iso file content
pub const ISOINFO: &str = "isoinfo";
/// socket file explorer
pub const SS: &str = "ss";
/// mkdir is used to create path before mounting
pub const MKDIR: &str = "mkdir";
/// mount is used to mount usb removable devices
pub const MOUNT: &str = "mount";
/// umount is used to mount usb removable devices
pub const UMOUNT: &str = "umount";
/// lsblk is used to get mountpoints, info about encrypted drives
pub const LSBLK: &str = "lsblk";
/// cryptsetup is used to mount encrypted drives
pub const CRYPTSETUP: &str = "cryptsetup";
/// udisksctl is used to mount drives
pub const UDISKSCTL: &str = "udisksctl";
/// gio is used to mount removable devices
pub const GIO: &str = "gio";
/// used to get information about fifo files
pub const UDEVADM: &str = "udevadm";
/// neovim executable
pub const NVIM: &str = "nvim";
/// bsdtar executable, used to display common archive content
pub const BSDTAR: &str = "bsdtar";
/// 7z executable, used to display 7z archive content
pub const SEVENZ: &str = "7z";
/// libreoffice executable
pub const LIBREOFFICE: &str = "libreoffice";
/// lazygit
pub const LAZYGIT: &str = "lazygit";
/// duf
pub const NCDU: &str = "ncdu";
/// transmission-show
pub const TRANSMISSION_SHOW: &str = "transmission-show";
/// Zoxide executable
pub const ZOXIDE: &str = "zoxide";
/// pdftoppm
pub const PDFTOPPM: &str = "pdftoppm";
/// pdfinfo
pub const PDFINFO: &str = "pdfinfo";
/// pdftotext
pub const PDFTOTEXT: &str = "pdftotext";
/// readelf
pub const READELF: &str = "readelf";
/// default nerdfont icon used for directories.
pub const DIR_ICON: &str = " ";
