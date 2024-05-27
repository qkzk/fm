use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};

use crate::log_info;
use crate::modes::FileInfo;
use crate::modes::Preview;
use crate::modes::Users;

#[derive(Clone)]
pub struct PreviewHolder {
    pub users: Users,
    pub is_previewing: bool,
    pub previews: Arc<Mutex<BTreeMap<PathBuf, Arc<Preview>>>>,
}

impl PreviewHolder {
    pub const MAX_SIZE: usize = 20;

    pub fn new() -> Self {
        let users = Users::new();
        let previews = Arc::new(Mutex::new(BTreeMap::new()));
        let is_previewing = false;
        Self {
            users,
            is_previewing,
            previews,
        }
    }

    pub fn get(&self, p: &std::path::Path) -> Option<Arc<Preview>> {
        log_info!("preview holder asked for {p}", p = p.display());
        let previews = self.previews.lock().ok()?;
        let ret = previews.get(p).cloned();
        log_info!(
            "PreviewHolder has {p}: {r}",
            p = p.display(),
            r = ret.is_some()
        );
        ret
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

    pub fn start_previewer(&mut self) -> bool {
        self.is_previewing = true;
        self.is_previewing
    }

    pub fn stop_previewer(&mut self) -> bool {
        self.is_previewing = false;
        self.is_previewing
    }

    pub fn build(&mut self, file_info: &FileInfo) -> Result<()> {
        let Ok(previews) = self.previews.lock() else {
            return Ok(());
        };
        if previews.contains_key(file_info.path.as_ref()) {
            return Ok(());
        }
        drop(previews);
        let file_info = file_info.to_owned();
        let preview_holder = self.previews.clone();
        let users = self.users.clone();
        log_info!("building preview for {file_info:?}");
        std::thread::spawn(move || -> Result<()> {
            let preview = Preview::new(&file_info, &users)?;
            log_info!("built preview for {file_info:?}");
            let Ok(mut preview_holder) = preview_holder.lock() else {
                return Ok(());
            };
            if !preview_holder.contains_key(file_info.path.as_ref()) {
                preview_holder.insert(file_info.path.to_path_buf(), Arc::new(preview));
                log_info!("inserted {file_info:?} in preview_holder");
            }
            Ok(())
        });
        Ok(())
    }
}
