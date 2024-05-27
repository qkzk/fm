use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};

use anyhow::{anyhow, Result};

use crate::log_info;
use crate::modes::FileInfo;
use crate::modes::Preview;
use crate::modes::Users;

pub enum PreviewCommand {
    Start,
    Stop,
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

    pub fn poll(&mut self) {
        while let Some(command) = self.rx.try_iter().next() {
            self.dispatch(command)
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

    pub fn dispatch(&mut self, command: PreviewCommand) {
        log_info!("dispatch...");
        match command {
            PreviewCommand::Start => {
                self.start_previewer();
            }
            PreviewCommand::Stop => {
                self.stop_previewer();
            }
            PreviewCommand::Paths(fileinfos) => {
                log_info!("received {fileinfos:?}");
                for file_info in fileinfos.iter() {
                    let _ = self.build(file_info);
                }
            }
        };
    }

    fn build(&mut self, file_info: &FileInfo) -> Result<()> {
        let file_info = file_info.to_owned();
        let preview_hodler = self.previews.clone();
        let users = self.users.clone();
        log_info!("building preview for {file_info:?}");
        std::thread::spawn(move || -> Result<()> {
            let preview = Preview::new(&file_info, &users)?;
            log_info!("built preview for {file_info:?}");
            if let Ok(mut preview_holder) = preview_hodler.lock() {
                if !preview_holder.contains_key(file_info.path.as_ref()) {
                    preview_holder.insert(file_info.path.to_path_buf(), Arc::new(preview));
                    log_info!("inserted {file_info:?} in preview_holder");
                }
            }
            Ok(())
        });
        Ok(())
    }
}
