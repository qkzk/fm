// TODO: numeroter chaque action de chaque mode,
// Dans chaque mode associer l'action en question
// Pour chaque mode générer un contenu par default ou lier le truc de status vers ça
// Pour certains modes (history, flagged, tui, cli, shortcut)
// En fait c'est vraiment utile pour ceux qui ont un default et sont pas mutables
// Compress, Bulk :/
use anyhow::Result;

use crate::impl_selectable_content;
use crate::modes::Navigate;

#[derive(Clone, Debug)]
pub struct Menu {
    kind: Navigate,
    content: Vec<String>,
    index: usize,
}

impl Default for Menu {
    fn default() -> Self {
        Self {
            kind: Navigate::Jump,
            content: vec![],
            index: 0,
        }
    }
}

impl Menu {
    pub fn reset(&mut self) {
        self.kind = Navigate::Jump;
        self.content = vec![];
        self.index = 0;
    }

    pub fn change_kind(&mut self, kind: Navigate, content: Vec<String>) {
        self.kind = kind;
        self.content = content;
        self.index = 0;
    }

    pub fn update_content(&mut self, content: Vec<String>) {
        let current = if !self.is_empty() {
            Some(self.content[self.index].to_owned())
        } else {
            None
        };
        self.content = content;
        if self.index >= self.content.len() {
            self.index = 0;
        }
        let Some(current) = current else {
            return;
        };
        let Some(index) = self.content.iter().position(|x| *x == current) else {
            return;
        };
        self.index = index;
    }

    pub fn append(&mut self, line: String) {
        self.content.push(line)
    }

    /// Remove the first occurence of `line` from menu
    pub fn remove(&mut self, line: String) {
        if let Some(pos) = self.content.iter().position(|x| *x == line) {
            self.content.remove(pos);
            if self.index >= self.len() {
                self.index -= 1;
            }
        }
    }

    pub fn kind_mut(&mut self) -> &mut Navigate {
        &mut self.kind
    }

    pub fn kind(&self) -> &Navigate {
        &self.kind
    }

    pub fn action(&mut self, action: u8) -> Result<()> {
        match self.kind {
            _ => todo!(),
        };
        Ok(())
    }
}

impl_selectable_content!(String, Menu);
