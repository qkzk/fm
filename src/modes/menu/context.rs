use std::fmt::Formatter;
use std::fs::Metadata;
use std::time::SystemTime;

use strum::{EnumIter, IntoEnumIterator};

use crate::event::ActionMap;
use crate::io::Opener;
use crate::modes::{extract_datetime, ExtensionKind, FileInfo};
use crate::{impl_content, impl_draw_menu_with_char, impl_selectable};

const CONTEXT_ACTIONS: [(&str, ActionMap); 10] = [
    ("Open", ActionMap::OpenFile),
    ("Open with", ActionMap::Exec),
    ("Open in Neovim", ActionMap::NvimFilepicker),
    ("Flag", ActionMap::ToggleFlag),
    ("Rename", ActionMap::Rename),
    ("Delete", ActionMap::Delete),
    ("Trash", ActionMap::TrashMoveFile),
    ("Chmod", ActionMap::Chmod),
    ("New File", ActionMap::NewFile),
    ("New Directory", ActionMap::NewDir),
];

/// Context menu of a file.
/// A few possible actions and some more information about this file.
#[derive(Default)]
pub struct ContextMenu {
    pub content: Vec<&'static str>,
    index: usize,
    actions: Vec<&'static ActionMap>,
}

impl ContextMenu {
    pub fn setup(&mut self) {
        self.content = CONTEXT_ACTIONS.iter().map(|(s, _)| *s).collect();
        self.actions = CONTEXT_ACTIONS.iter().map(|(_, a)| a).collect();
    }

    pub fn matcher(&self) -> &ActionMap {
        self.actions[self.index]
    }

    pub fn reset(&mut self) {
        self.index = 0;
    }
}

type StaticStr = &'static str;

impl_content!(ContextMenu, StaticStr);
impl_draw_menu_with_char!(ContextMenu, StaticStr);

/// Used to generate more informations about a file in the context menu.
pub struct MoreInfos<'a> {
    file_info: &'a FileInfo,
    opener: &'a Opener,
}

impl<'a> MoreInfos<'a> {
    pub fn new(file_info: &'a FileInfo, opener: &'a Opener) -> Self {
        Self { file_info, opener }
    }

    /// Informations about the file as an array of strings.
    pub fn to_lines(&self) -> [String; 7] {
        let mut times = self.system_times();
        [
            self.owner_group(),
            self.perms(),
            self.size_inode(),
            std::mem::take(&mut times[0]),
            std::mem::take(&mut times[1]),
            std::mem::take(&mut times[2]),
            self.kind_opener(),
        ]
    }

    fn owner_group(&self) -> String {
        format!(
            "Owner/Group: {owner} / {group}",
            owner = self.file_info.owner,
            group = self.file_info.group
        )
    }

    fn perms(&self) -> String {
        if let Ok(perms) = self.file_info.permissions() {
            format!(
                "Permissions: {dir_symbol}{perms}",
                dir_symbol = self.file_info.dir_symbol()
            )
        } else {
            "".to_owned()
        }
    }

    fn size_inode(&self) -> String {
        format!(
            "{size_kind} {size} / Inode: {inode}",
            size_kind = self.file_info.file_kind.size_description(),
            size = self.file_info.size_column.trimed(),
            inode = self.file_info.ino()
        )
    }

    fn kind_opener(&self) -> String {
        if self.file_info.file_kind.is_normal_file() {
            let ext_kind = ExtensionKind::matcher(&self.file_info.extension.to_lowercase());
            if let Some(opener) = self.opener.kind(&self.file_info.path) {
                format!("Opener: {opener}, Previewer: {ext_kind}")
            } else {
                format!("Previewer:  {ext_kind}")
            }
        } else {
            let kind = self.file_info.file_kind.long_description();
            format!("Kind:        {kind}")
        }
    }

    fn system_times(&self) -> Vec<String> {
        let Ok(metadata) = &self.file_info.metadata() else {
            return vec!["".to_owned(), "".to_owned(), "".to_owned()];
        };
        TimeKind::iter()
            .map(|time_kind| time_kind.format_time(metadata))
            .collect()
    }
}

#[derive(EnumIter)]
enum TimeKind {
    Modified,
    Created,
    Accessed,
}

impl TimeKind {
    fn read_time(&self, metadata: &Metadata) -> Result<SystemTime, std::io::Error> {
        match self {
            Self::Modified => metadata.modified(),
            Self::Created => metadata.created(),
            Self::Accessed => metadata.accessed(),
        }
    }

    fn format_time(&self, metadata: &Metadata) -> String {
        let Ok(dt) = self.read_time(metadata) else {
            return "".to_owned();
        };
        let formated_time = extract_datetime(dt).unwrap_or_default();
        format!("{self}{formated_time}")
    }
}

impl std::fmt::Display for TimeKind {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            Self::Modified => write!(f, "Modified:    ",),
            Self::Created => write!(f, "Created:     ",),
            Self::Accessed => write!(f, "Assessed:    "),
        }
    }
}
