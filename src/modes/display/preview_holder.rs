use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};

use anyhow::{anyhow, Result};

use crate::modes::FileInfo;
use crate::modes::Preview;
use crate::modes::Users;

pub enum PreviewCommand {
    Toggle,
    Paths(Vec<FileInfo>),
}

pub struct PreviewHolder {
    pub users: Users,
    pub is_previewing: bool,
    pub rx: mpsc::Receiver<PreviewCommand>,
    pub previews: Arc<Mutex<BTreeMap<PathBuf, Arc<Preview>>>>,
}

impl PreviewHolder {
    pub const MAX_SIZE: usize = 20;

    pub fn new(rx: mpsc::Receiver<PreviewCommand>) -> Self {
        let users = Users::new();
        let previews = Arc::new(Mutex::new(BTreeMap::new()));
        let is_previewing = false;
        Self {
            users,
            is_previewing,
            rx,
            previews,
        }
    }

    pub fn get(&self, p: &std::path::Path) -> Option<Arc<Preview>> {
        let previews = self.previews.lock().ok()?;
        previews.get(p).cloned()
    }

    pub fn size(&mut self) -> Result<usize> {
        let Ok(previews) = self.previews.lock() else {
            return Err(anyhow!("Couldn't lock preview holder"));
        };
        Ok(previews.len())
    }

    pub fn clear(&mut self) -> Result<()> {
        let Ok(mut previews) = self.previews.lock() else {
            return Err(anyhow!("Couldn't lock preview holder"));
        };
        previews.clear();
        Ok(())
    }

    pub fn toggle_previewer(&mut self) -> bool {
        self.is_previewing = !self.is_previewing;
        if !self.is_previewing {
            let _ = self.clear();
        }
        self.is_previewing
    }

    pub fn dispatch(&mut self, command: PreviewCommand) {
        match command {
            PreviewCommand::Toggle => {
                self.toggle_previewer();
            }
            PreviewCommand::Paths(fileinfos) => {
                if !self.is_previewing {
                    return;
                };
                fileinfos.iter().for_each(|file_info| {
                    let _ = self.build(file_info);
                })
            }
        };
    }

    fn build(&mut self, file_info: &FileInfo) -> Result<()> {
        let file_info = file_info.to_owned();
        let preview_hodler = self.previews.clone();
        let users = self.users.clone();
        std::thread::spawn(move || -> Result<()> {
            let preview = Preview::new(&file_info, &users)?;
            if let Ok(mut preview_holder) = preview_hodler.lock() {
                if !preview_holder.contains_key(file_info.path.as_ref()) {
                    preview_holder.insert(file_info.path.to_path_buf(), Arc::new(preview));
                }
            }
            Ok(())
        });
        Ok(())
    }
}
