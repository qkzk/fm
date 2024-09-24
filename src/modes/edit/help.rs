use anyhow::Result;
use strfmt::strfmt;

use crate::config::Bindings;
use crate::event::ActionMap;
use crate::io::Opener;

const CUSTOM_HELP: &str = "
- CUSTOM ACTIONS -
%s: the selected file,
%f: the flagged files,
%e: the extension of the file,
%n: the filename only,
%p: the full path of the current directory.
";

/// Creates the help `String` from keybindings.
/// If multiple keybindings are bound to the same action, the last one
/// is displayed.
/// If an action displayed in help isn't bound to a key, the formating won't
/// be possible. We use the default keybindings instead.
/// If it doesn't work, we return an empty string.
pub fn help_string(binds: &Bindings, opener: &Opener) -> String {
    match make_help_with_config(binds, opener) {
        Ok(help) => help,
        Err(error) => {
            crate::log_info!("Error parsing help: {error}");
            let mut help = format!(
                "Couldn't parse your keybindings: {error}.
Using default keybindings.

"
            );
            help.push_str(&make_help_with_config(&Bindings::new(), opener).unwrap_or_default());
            help
        }
    }
}

macro_rules! action_descriptions {
    ( $( $name:ident ),* $(,)? ) => {
        format!(
            concat!(
                $(
                    "{{", stringify!($name), ":<10}}:      {}\n",
                )*
            ),
            $(
                ActionMap::$name.description(),
            )*
        )
    };
}

/// Help message to be displayed when help key is pressed.
/// Default help key is `'h'`.
fn format_help_message() -> String {
    format!(
        "
{quit}

- Navigation -
{navigation}

- Actions -
{actions}
    - default       {{Default}}
    - audio         {{Audio}}
    - images        {{Bitmap}}
    - office        {{Office}}
    - pdf, ebooks   {{Readable}}
    - text          {{Text}}
    - video         {{Video}}
    - vectorials    {{Vectorial}}
    - compressed files are decompressed
    - iso images are mounted

{more_actions}

- Action on flagged files -
{flagged_actions}

- Trash -
{trash_actions}

- Tree -
Navigate as usual. Most actions work as in 'normal' view.

{tree_actions}

    - DISPLAY MODES -
Different modes for the main window
{display_modes}

    - EDIT MODES -
Different modes for the bottom window
{edit_modes}
",
        quit = action_descriptions!(Quit, Help),
        navigation = action_descriptions!(
            MoveLeft, MoveRight, MoveUp, MoveDown, KeyHome, End, PageUp, PageDown, Tab
        ),
        actions = action_descriptions!(
            ToggleDualPane,
            TogglePreviewSecond,
            ToggleDisplayFull,
            ToggleHidden,
            Shell,
            OpenFile,
            NvimFilepicker,
            NvimSetAddress,
            Preview,
            Back,
            Home,
            GoRoot,
            GoStart,
            MarksNew,
            MarksJump,
            SearchNext,
            FuzzyFind,
            FuzzyFindLine,
            FuzzyFindHelp,
            RefreshView,
            CopyFilename,
            CopyFilepath,
            OpenConfig,
            CloudDrive,
        ),
        more_actions = action_descriptions!(Action),
        flagged_actions = action_descriptions!(
            ToggleFlag,
            FlagAll,
            ClearFlags,
            ReverseFlags,
            Symlink,
            CopyPaste,
            CutPaste,
            Delete,
            TrashMoveFile,
            Compress,
            FlaggedToClipboard,
            FlaggedFromClipboard
        ),
        trash_actions = action_descriptions!(TrashOpen, TrashEmpty),
        tree_actions = action_descriptions!(Tree, TreeFold, TreeFoldAll, TreeUnFoldAll),
        display_modes = action_descriptions!(ResetMode, Tree, Preview),
        edit_modes = action_descriptions!(
            Chmod,
            Exec,
            NewDir,
            NewFile,
            Rename,
            Cd,
            RegexMatch,
            Sort,
            History,
            Shortcut,
            EncryptedDrive,
            RemovableDevices,
            Search,
            Action,
            Bulk,
            TuiMenu,
            CliMenu,
            RemoteMount,
            Filter,
            DisplayFlagged,
            Context,
            Enter
        ),
    )
}

fn make_help_with_config(binds: &Bindings, opener: &Opener) -> Result<String> {
    let mut keybind_reversed = binds.keybind_reversed();
    keybind_reversed.extend(opener.association.as_map_of_strings());
    let mut help = strfmt(&format_help_message(), &keybind_reversed)?;
    help = complete_with_custom_binds(&binds.custom, help);
    // std::fs::write("help.txt", &help)?; // keep here to save a new version of the help content
    Ok(help)
}

fn complete_with_custom_binds(custom_binds: &Option<Vec<String>>, mut help: String) -> String {
    if let Some(customs) = &custom_binds {
        help.push_str(CUSTOM_HELP);
        for custom in customs {
            help.push_str(custom);
        }
    }
    help
}
