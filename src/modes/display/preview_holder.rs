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
        let Ok(previews) = self.previews.lock() else {
            log_info!("PreviewHolder.get: couldn't acquire lock");
            return None;
        };
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

    pub fn start_previewer(&mut self) -> bool {
        self.is_previewing = true;
        self.is_previewing
    }

    pub fn stop_previewer(&mut self) -> bool {
        self.is_previewing = false;
        self.is_previewing
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
        let Ok(previews) = self.previews.lock() else {
            log_info!("PreviewHolder.build couldn't acquire lock");
            return Ok(());
        };
        if previews.contains_key(path) {
            return Ok(());
        }
        drop(previews);
        let preview_holder = self.previews.clone();
        let users = self.users.clone();
        log_info!("building preview for {path}", path = path.display());
        let pathb = path.to_owned();

        std::thread::spawn(move || -> Result<()> {
            let preview = Preview::new(pathb.as_path(), &users)?;
            let Ok(mut preview_holder) = preview_holder.lock() else {
                log_info!("PreviewHolder.build thread. Couldn't acquire lock");
                return Ok(());
            };
            if !preview_holder.contains_key(&pathb) {
                log_info!("inserted {p} in preview_holder", p = pathb.display());
                preview_holder.insert(pathb, Arc::new(preview));
            }
            Ok(())
        });
        Ok(())
    }

    pub fn build_directory(&mut self, files: Vec<PathBuf>) -> Result<()> {
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
                        "build_directory. {path} inserted in preview_holder",
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
