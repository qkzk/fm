use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};

use crate::log_info;
use crate::modes::Preview;
use crate::modes::Users;

#[derive(Clone)]
pub struct PreviewHolder {
    pub users: Users,
    pub previews: Arc<Mutex<BTreeMap<PathBuf, Arc<Preview>>>>,
}

impl PreviewHolder {
    const MAX_SIZE: usize = 1000;

    pub fn new() -> Self {
        let users = Users::new();
        let previews = Arc::new(Mutex::new(BTreeMap::new()));
        Self { users, previews }
    }

    pub fn get(&self, p: &std::path::Path) -> Option<Arc<Preview>> {
        let Ok(previews) = self.previews.lock() else {
            log_info!("PreviewHolder.get: couldn't acquire lock");
            return None;
        };
        previews.get(p).cloned()
    }

    pub fn size(&mut self) -> Result<usize> {
        let Ok(previews) = self.previews.lock() else {
            return Err(anyhow!("PreviewHolder.size. Couldn't lock preview holder"));
        };
        Ok(previews.len())
    }

    pub fn clear(&mut self) -> Result<()> {
        let Ok(mut previews) = self.previews.lock() else {
            return Err(anyhow!("PreviewHolder.clear. Couldn't lock preview holder"));
        };
        previews.clear();
        Ok(())
    }

    pub fn put_preview<P>(&mut self, path: P, preview: Preview)
    where
        P: AsRef<std::path::Path>,
    {
        let Ok(mut previews) = self.previews.lock() else {
            log_info!("PreviewHolder.put_preview: couldn't acquire lock");
            return;
        };
        previews.insert(path.as_ref().to_owned(), Arc::new(preview));
    }

    pub fn build(&mut self, path: &std::path::Path) -> Result<()> {
        let Ok(mut previews) = self.previews.lock() else {
            log_info!("PreviewHolder.build couldn't acquire lock");
            return Ok(());
        };
        if previews.contains_key(path) {
            return Ok(());
        }
        if previews.len() >= Self::MAX_SIZE {
            previews.clear()
        }
        drop(previews);
        let preview_holder = Arc::clone(&self.previews);
        let users = self.users.clone();
        let path = path.to_owned();
        std::thread::spawn(move || -> Result<()> {
            let preview = Preview::new(path.as_path(), &users)?;
            let Ok(mut preview_holder) = preview_holder.lock() else {
                return Ok(());
            };
            if !preview_holder.contains_key(&path) {
                log_info!("inserted {p} in preview_holder", p = path.display());
                preview_holder.insert(path, Arc::new(preview));
            }
            Ok(())
        });
        Ok(())
    }

    pub fn build_collection(&mut self, files: Vec<PathBuf>) -> Result<()> {
        let preview_holder = self.previews.clone();
        let users = self.users.clone();
        std::thread::spawn(move || -> Result<()> {
            for path in files {
                let preview = Preview::new(&path, &users)?;
                let Ok(mut preview_holder) = preview_holder.lock() else {
                    log_info!("PreviewHolder.build_directory Couldn't acquire lock");
                    return Ok(());
                };
                if !preview_holder.contains_key(&path) {
                    log_info!(
                        "build_collection. {path} inserted in preview_holder",
                        path = path.display()
                    );
                    preview_holder.insert(path, Arc::new(preview));
                }
            }
            Ok(())
        });
        Ok(())
    }
}
