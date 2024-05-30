use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use parking_lot::RwLock;

use crate::log_info;
use crate::modes::Preview;
use crate::modes::Ueberzug;
use crate::modes::Users;

#[derive(Clone)]
pub struct PreviewHolder {
    pub users: Users,
    pub previews: Arc<RwLock<BTreeMap<PathBuf, Arc<Preview>>>>,
}

impl PreviewHolder {
    const MAX_SIZE: usize = 1000;

    pub fn new() -> Self {
        let users = Users::new();
        let previews = Arc::new(RwLock::new(BTreeMap::new()));
        Self { users, previews }
    }

    pub fn get(&self, p: &std::path::Path) -> Option<Arc<Preview>> {
        self.previews.read().get(p).cloned()
    }

    pub fn clear(&mut self) -> Result<()> {
        self.previews.write().clear();
        Ok(())
    }

    pub fn put_preview<P>(&mut self, path: P, preview: Preview)
    where
        P: AsRef<std::path::Path>,
    {
        self.previews
            .write()
            .insert(path.as_ref().to_owned(), Arc::new(preview));
    }

    pub fn build(&mut self, path: &std::path::Path, ueberzug: Arc<Ueberzug>) -> Result<()> {
        if self.previews.read().contains_key(path) {
            return Ok(());
        }
        if self.previews.read().len() >= Self::MAX_SIZE {
            self.previews.write().clear()
        }
        let preview_holder = Arc::clone(&self.previews);
        let users = self.users.clone();
        let path = path.to_owned();
        let Ok(preview) = Preview::new(path.as_path(), &users, ueberzug) else {
            log_info!("Couldn't build preview for {path}", path = path.display());
            return Ok(());
        };
        if preview_holder.read().contains_key(&path) {
            return Ok(());
        }
        log_info!("inserted {p} in preview_holder", p = path.display());
        preview_holder.write().insert(path, Arc::new(preview));
        Ok(())
    }

    pub fn build_collection(&mut self, files: Vec<PathBuf>, ueberzug: Arc<Ueberzug>) -> Result<()> {
        let preview_holder = self.previews.clone();
        let users = self.users.clone();
        std::thread::spawn(move || -> Result<()> {
            for path in files {
                let Ok(preview) = Preview::new(&path, &users, ueberzug.clone()) else {
                    log_info!("Couldn't build preview for {path}", path = path.display());
                    return Ok(());
                };
                if preview_holder.read().contains_key(&path) {
                    return Ok(());
                }
                log_info!(
                    "build_collection. {path} inserted in preview_holder",
                    path = path.display()
                );
                preview_holder.write().insert(path, Arc::new(preview));
            }
            Ok(())
        });
        Ok(())
    }

    pub fn hide_all_images(&mut self) {
        self.previews
            .read()
            .values()
            .for_each(|preview| preview.hide())
    }
}
