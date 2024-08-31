use anyhow::Result;
use opendal::services;
use opendal::Entry;
use opendal::Operator;
use serde::Deserialize;

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
pub async fn google_drive() -> Result<()> {
    let google_drive_config = read_google_drive_config("google_drive_token.yaml").await?;
    let op = create_google_drive_operator(&google_drive_config).await?;

    // List all files and directories at the root level.
    let entries = op.list(&google_drive_config.root_folder).await?;

    log_info!(
        "Google drive: {name} - {folder}",
        name = google_drive_config.drive_name,
        folder = google_drive_config.root_folder,
    );
    // Iterate over the list of files/directories.

    let entries_container = EntriesContainer::new(
        OpendalKind::GoogleDrive,
        &google_drive_config.drive_name,
        entries,
    );

    for entry in entries_container.content.iter() {
        log_info!("Found: {}", entry.path());
        log_info!("metadata {:?}", entry.metadata());
    }

    Ok(())
}

pub enum OpendalKind {
    GoogleDrive,
}

impl OpendalKind {
    pub fn repr(&self) -> &'static str {
        match self {
            &Self::GoogleDrive => "google_drive",
        }
    }
}

pub struct EntriesContainer {
    pub kind: OpendalKind,
    pub path: std::path::PathBuf,
    pub index: usize,
    pub content: Vec<Entry>,
}

impl EntriesContainer {
    pub fn new(kind: OpendalKind, drive_name: &str, content: Vec<Entry>) -> Self {
        Self {
            path: std::path::PathBuf::from(&format!(
                "{kind_format}/{drive_name}/",
                kind_format = kind.repr()
            )),
            kind,
            index: 0,
            content,
        }
    }
}

impl_selectable!(EntriesContainer);
impl_content!(Entry, EntriesContainer);
