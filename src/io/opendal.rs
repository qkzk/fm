use anyhow::anyhow;
use anyhow::Result;
use opendal::services;
use opendal::Entry;
use opendal::EntryMode;
use opendal::Operator;
use serde::Deserialize;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use crate::common::path_to_string;
use crate::common::CONFIG_FOLDER;
use crate::impl_content;
use crate::impl_selectable;
use crate::log_info;
use crate::log_line;
use crate::modes::FileInfo;

#[derive(Deserialize, Debug)]
struct GoogleDriveConfig {
    drive_name: String,
    root_folder: String,
    refresh_token: String,
    client_id: String,
    client_secret: String,
}

fn build_token_filename(config_name: &str) -> String {
    let token_base_path = shellexpand::tilde(CONFIG_FOLDER);
    format!("{token_base_path}/token_{config_name}.yaml")
}

/// Read the token & root folder from the token file.
async fn read_google_drive_config(config_name: &str) -> Result<GoogleDriveConfig> {
    let config_filename = build_token_filename(config_name);
    let token_data = tokio::fs::read_to_string(config_filename).await?;
    let google_drive_token: GoogleDriveConfig = serde_yaml::from_str(&token_data)?;
    log_info!("config {google_drive_token:?}");
    Ok(google_drive_token)
}

/// Set up the Google Drive backend.
async fn create_google_drive_operator(google_drive_config: &GoogleDriveConfig) -> Result<Operator> {
    let builder = services::Gdrive::default()
        .refresh_token(&google_drive_config.refresh_token)
        .client_id(&google_drive_config.client_id)
        .client_secret(&google_drive_config.client_secret)
        .root(&google_drive_config.root_folder);

    let op = Operator::new(builder)?.finish();
    Ok(op)
}

#[tokio::main]
pub async fn google_drive(token_file: &str) -> Result<OpendalContainer> {
    let google_drive_config = read_google_drive_config(token_file).await?;
    log_info!("found google_drive_config");
    let op = create_google_drive_operator(&google_drive_config).await?;

    // List all files and directories at the root level.
    let entries = op.list(&google_drive_config.root_folder).await?;

    // Create the container
    let opendal_container = OpendalContainer::new(
        op,
        OpendalKind::GoogleDrive,
        &google_drive_config.drive_name,
        &google_drive_config.root_folder,
        entries,
    );

    Ok(opendal_container)
}

pub enum OpendalKind {
    Empty,
    GoogleDrive,
}

impl OpendalKind {
    pub fn repr(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::GoogleDrive => "Google Drive",
        }
    }
}

pub fn entry_mode_fmt(entry: &Entry) -> &'static str {
    match entry.metadata().mode() {
        EntryMode::Unknown => "? ",
        EntryMode::DIR => "D ",
        EntryMode::FILE => "F ",
    }
}

pub struct OpendalContainer {
    pub op: Option<Operator>,
    pub kind: OpendalKind,
    desc: String,
    pub path: std::path::PathBuf,
    pub root: std::path::PathBuf,
    pub index: usize,
    pub content: Vec<Entry>,
}

impl Default for OpendalContainer {
    fn default() -> Self {
        Self {
            op: None,
            kind: OpendalKind::Empty,
            desc: "empty".to_owned(),
            path: std::path::PathBuf::from(""),
            root: std::path::PathBuf::from(""),
            index: 0,
            content: vec![],
        }
    }
}

impl OpendalContainer {
    pub fn new(
        op: Operator,
        kind: OpendalKind,
        drive_name: &str,
        root_path: &str,
        content: Vec<Entry>,
    ) -> Self {
        Self {
            op: Some(op),
            desc: format!("{kind_format}/{drive_name}", kind_format = kind.repr()),
            path: std::path::PathBuf::from(root_path),
            root: std::path::PathBuf::from(root_path),
            kind,
            index: 0,
            content,
        }
    }

    pub fn is_set(&self) -> bool {
        self.op.is_some()
    }

    fn cloud_build_dest_filename(&self, local_file: &FileInfo) -> String {
        let filename = local_file.filename.as_ref();
        let mut dest_path = self.path.clone();
        dest_path.push(filename);
        path_to_string(&dest_path)
    }

    pub async fn upload(&self, local_file: &FileInfo) -> Result<()> {
        let Some(op) = &self.op else {
            return Ok(());
        };
        let dest_path_str = self.cloud_build_dest_filename(local_file);
        let bytes = tokio::fs::read(&local_file.path).await?;
        op.write(&dest_path_str, bytes).await?;
        log_line!(
            "Uploaded {filename} to {path}",
            filename = local_file.filename,
            path = self.path.display()
        );
        Ok(())
    }

    pub fn selected_filename(&self) -> Option<&str> {
        self.selected()?.path().split('/').last()
    }

    fn create_downloaded_path(&self, dest: &str) -> Option<std::path::PathBuf> {
        let distant_filename = self.selected_filename()?;
        let mut dest = std::path::PathBuf::from(dest);
        dest.push(distant_filename);
        if dest.exists() {
            log_info!(
                "Local file {dest} already exists. Can't download here",
                dest = dest.display()
            );
            log_line!("Local file {dest} already exists. Choose another path or rename the existing file first.", dest=dest.display());
            None
        } else {
            Some(dest)
        }
    }

    pub async fn download(&self, dest: &str) -> Result<()> {
        let Some(op) = &self.op else {
            return Ok(());
        };
        let Some(selected) = self.selected() else {
            return Ok(());
        };
        let distant_filepath = selected.path();
        let Some(dest) = self.create_downloaded_path(dest) else {
            return Ok(());
        };
        let buf = op.read(distant_filepath).await?;
        let mut file = File::create(&dest).await?;
        file.write_all(&buf.to_bytes()).await?;
        log_info!(
            "Downloaded {distant_filepath} to local file {dest}",
            dest = dest.display(),
        );
        Ok(())
    }

    pub async fn create_newdir(&mut self, dirname: String) -> Result<()> {
        let current_path = &self.path;
        let Some(op) = &self.op else {
            return Err(anyhow!("Cloud container has no operator"));
        };
        let fp = current_path.join(dirname);
        let mut fullpath = path_to_string(&fp);
        if !fullpath.ends_with('/') {
            fullpath.push('/');
        }
        op.create_dir(&fullpath).await?;
        Ok(())
    }

    pub fn disconnect(&mut self) {
        let desc = self.desc.to_owned();
        self.op = None;
        self.kind = OpendalKind::Empty;
        self.desc = "empty".to_owned();
        self.path = std::path::PathBuf::from("");
        self.root = std::path::PathBuf::from("");
        self.index = 0;
        self.content = vec![];
        log_info!("Disconnected from {desc}");
    }

    pub async fn delete(&mut self) -> Result<()> {
        let Some(op) = &self.op else {
            return Ok(());
        };
        let Some(entry) = self.selected() else {
            return Ok(());
        };
        let file_to_delete = entry.path();
        op.delete(file_to_delete).await?;
        log_info!("Deleted {file_to_delete}");
        log_line!("Deleted {file_to_delete}");
        Ok(())
    }

    #[tokio::main]
    pub async fn update_path(&mut self, path: &str) -> Result<()> {
        if let Some(op) = &self.op {
            self.content = op.list(path).await?;
            self.path = std::path::PathBuf::from(path);
            self.index = 0;
        };
        Ok(())
    }

    fn ensure_index_in_bounds(&mut self) {
        self.index = std::cmp::min(self.content.len().saturating_sub(1), self.index)
    }

    pub async fn refresh_current(&mut self) -> Result<()> {
        let Some(op) = &self.op else {
            return Ok(());
        };
        self.content = op.list(&path_to_string(&self.path)).await?;
        self.ensure_index_in_bounds();
        Ok(())
    }

    #[tokio::main]
    pub async fn move_to_parent(&mut self) -> Result<()> {
        if let Some(op) = &self.op {
            if self.path == self.root {
                return Ok(());
            };
            if let Some(parent) = self.path.to_owned().parent() {
                self.path = parent.to_path_buf();
                self.content = op.list(&path_to_string(&parent)).await?;
                self.index = 0;
            }
        }
        Ok(())
    }

    pub fn desc(&self) -> String {
        format!(
            "{desc}{sep}{path}",
            desc = self.desc,
            sep = if self.path == self.root { "" } else { "/" },
            path = self.path.display()
        )
    }
}

impl_selectable!(OpendalContainer);
impl_content!(Entry, OpendalContainer);
