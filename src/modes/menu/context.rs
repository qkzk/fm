use crate::event::ActionMap;
use crate::io::Opener;
use crate::modes::{extract_datetime, ExtensionKind, FileInfo, FileKind};
use crate::{impl_content, impl_draw_menu_with_char, impl_selectable};

const CONTEXT: [(&str, ActionMap); 10] = [
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
        self.content = CONTEXT.iter().map(|(s, _)| *s).collect();
        self.actions = CONTEXT.iter().map(|(_, a)| a).collect();
    }

    pub fn matcher(&self) -> &ActionMap {
        self.actions[self.index]
    }

    pub fn reset(&mut self) {
        self.index = 0;
    }
}

type StaticStr = &'static str;

impl_selectable!(ContextMenu);
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

    /// Informations about the file as a vector of string.
    pub fn to_lines(&self) -> Vec<String> {
        let mut lines = vec![];

        self.owner_group(&mut lines);
        self.perms(&mut lines);
        self.size(&mut lines);
        self.times(&mut lines);
        self.opener(&mut lines);
        self.kind(&mut lines);

        lines
    }

    fn owner_group(&self, lines: &mut Vec<String>) {
        lines.push(format!(
            "Owner/Group: {owner} / {group}",
            owner = self.file_info.owner,
            group = self.file_info.group
        ));
    }

    fn perms(&self, lines: &mut Vec<String>) {
        if let Ok(perms) = self.file_info.permissions() {
            lines.push(format!(
                "Permissions: {dir_symbol}{perms}",
                dir_symbol = self.file_info.dir_symbol()
            ));
        }
    }

    fn size(&self, lines: &mut Vec<String>) {
        lines.push(format!(
            "{size_kind} {size}",
            size_kind = self.file_info.file_kind.size_description(),
            size = self.file_info.size_column.trimed()
        ));
    }

    fn times(&self, lines: &mut Vec<String>) {
        if let Ok(metadata) = std::fs::metadata(&self.file_info.path) {
            if let Ok(created) = metadata.created() {
                if let Ok(dt) = extract_datetime(created) {
                    lines.push(format!("Created:     {dt}"))
                }
            }
            if let Ok(accessed) = metadata.accessed() {
                if let Ok(dt) = extract_datetime(accessed) {
                    lines.push(format!("Accessed:    {dt}"))
                }
            }
            if let Ok(modified) = metadata.modified() {
                if let Ok(dt) = extract_datetime(modified) {
                    lines.push(format!("Modified:    {dt}"))
                }
            }
        }
    }

    fn opener(&self, lines: &mut Vec<String>) {
        if let Some(opener) = self.opener.kind(&self.file_info.path) {
            lines.push(format!("Opener:      {opener}"));
        };
    }

    fn kind(&self, lines: &mut Vec<String>) {
        if matches!(self.file_info.file_kind, FileKind::NormalFile) {
            let ext_kind = ExtensionKind::matcher(&self.file_info.extension.to_lowercase());
            lines.push(format!("Previewer:   {ext_kind}"));
        } else {
            let kind = self.file_info.file_kind.long_description();
            lines.push(format!("Kind:        {kind}"));
        }
    }
}
