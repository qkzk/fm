use anyhow::Result;
use opendal::services;
use opendal::Entry;
use opendal::EntryMode;
use opendal::Operator;
use serde::Deserialize;

use crate::common::path_to_string;
use crate::impl_content;
use crate::impl_selectable;
use crate::log_info;

#[derive(Deserialize, Debug)]
struct GoogleDriveConfig {
    drive_name: String,
    root_folder: String,
    // access_token: String,
    refresh_token: String,
    client_id: String,
    client_secret: String,
}

/// Read the token & root folder from the token file.
async fn read_google_drive_config(config_file: &str) -> Result<GoogleDriveConfig> {
    let token_data = tokio::fs::read_to_string(config_file).await?;
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
        // .access_token(&google_drive_config.access_token)
        .root(&google_drive_config.root_folder);

    let op = Operator::new(builder)?.finish();
    Ok(op)
}

#[tokio::main]
pub async fn google_drive() -> Result<OpendalContainer> {
    let google_drive_config = read_google_drive_config("google_drive_token.yaml").await?;
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

    for entry in opendal_container.content.iter() {
        log_info!("Found: {}", entry.path());
        log_info!("metadata {:?}", entry.metadata());
    }

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

    #[tokio::main]
    pub async fn update_path(&mut self, path: &str) -> Result<()> {
        if let Some(op) = &self.op {
            self.content = op.list(path).await?;
            self.path = std::path::PathBuf::from(path);
            self.index = 0;
        };
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
            "{d}{sep}{p}",
            d = self.desc,
            sep = if self.path == self.root { "" } else { "/" },
            p = self.path.display()
        )
    }
}

impl_selectable!(OpendalContainer);
impl_content!(Entry, OpendalContainer);
