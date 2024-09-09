use anyhow::anyhow;
use anyhow::Context;
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
use crate::modes::human_size;
use crate::modes::FileInfo;

#[derive(Deserialize, Debug)]
struct GoogleDriveConfig {
    drive_name: String,
    root_folder: String,
    refresh_token: String,
    client_id: String,
    client_secret: String,
}

impl GoogleDriveConfig {
    fn build_token_filename(config_name: &str) -> String {
        let token_base_path = shellexpand::tilde(CONFIG_FOLDER);
        format!("{token_base_path}/token_{config_name}.yaml")
    }

    /// Read the token & root folder from the token file.
    async fn from_config(config_name: &str) -> Result<Self> {
        let config_filename = Self::build_token_filename(config_name);
        let token_data = tokio::fs::read_to_string(&config_filename).await?;
        let google_drive_token: Self = serde_yaml::from_str(&token_data)?;
        Ok(google_drive_token)
    }

    /// Set up the Google Drive backend.
    async fn build_operator(&self) -> Result<Operator> {
        let builder = services::Gdrive::default()
            .refresh_token(&self.refresh_token)
            .client_id(&self.client_id)
            .client_secret(&self.client_secret)
            .root(&self.root_folder);

        let op = Operator::new(builder)?.finish();
        Ok(op)
    }
}

/// Builds a google drive opendal container from a token filename.
#[tokio::main]
pub async fn google_drive(token_file: &str) -> Result<OpendalContainer> {
    let google_drive_config = GoogleDriveConfig::from_config(token_file).await?;
    log_info!("found google drive config {token_file}");
    let op = google_drive_config.build_operator().await?;
    log_info!("created operator");

    // List all files and directories at the root level.
    // let entries = op.list(&google_drive_config.root_folder).await?;
    let entries = match op.list(&google_drive_config.root_folder).await {
        Ok(entries) => entries,
        Err(err) => {
            log_info!("Error: {err:?}");
            return Err(anyhow!("error: {err:?}"));
        }
    };
    log_info!("listed entries");

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

/// Different kind of opendal container
#[derive(Default)]
pub enum OpendalKind {
    #[default]
    Empty,
    GoogleDrive,
}

impl OpendalKind {
    fn repr(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::GoogleDrive => "Google Drive",
        }
    }
}

/// Formating used to display elements.
pub trait ModeFormat {
    fn mode_fmt(&self) -> &'static str;
}

impl ModeFormat for Entry {
    fn mode_fmt(&self) -> &'static str {
        match self.metadata().mode() {
            EntryMode::Unknown => "? ",
            EntryMode::DIR => "d ",
            EntryMode::FILE => ". ",
        }
    }
}

/// Holds any relevant content of an opendal container.
/// It has an operator, allowing action on the remote files and knows
/// about the root path and current content.
#[derive(Default)]
pub struct OpendalContainer {
    /// Operator executing requests
    op: Option<Operator>,
    /// What kind of OpenDal container is it ?
    /// ATM only GoogleDrive and Unknown
    kind: OpendalKind,
    /// Friendly name of the container to be displayed
    name: String,
    /// Current path in the cloud
    path: std::path::PathBuf,
    /// Configured root path
    root: std::path::PathBuf,
    /// Current index
    pub index: usize,
    /// Retrieved files
    pub content: Vec<Entry>,
    /// Last retrieved information
    /// We keep a pair: index and string.
    /// It may be cached in the future
    pub metadata_repr: Option<(usize, String)>,
}

impl OpendalContainer {
    fn new(
        op: Operator,
        kind: OpendalKind,
        drive_name: &str,
        root_path: &str,
        content: Vec<Entry>,
    ) -> Self {
        Self {
            op: Some(op),
            name: format!("{kind_format}/{drive_name}", kind_format = kind.repr()),
            path: std::path::PathBuf::from(root_path),
            root: std::path::PathBuf::from(root_path),
            kind,
            index: 0,
            content,
            metadata_repr: None,
        }
    }

    fn selected_filepath(&self) -> Option<String> {
        Some(format!(
            "{path}{sep}{filename}",
            path = self.path.display(),
            sep = if self.path == self.root { "" } else { "/" },
            filename = self.selected_filename()?,
        ))
    }

    /// Update the metadata with for the currently selected file
    #[tokio::main]
    pub async fn update_metadata(&mut self) -> Result<()> {
        let Some(op) = &self.op else {
            return Ok(());
        };
        let Some(filename) = self.selected_filename() else {
            return Ok(());
        };
        let metadata = op
            .stat_with(&self.selected_filepath().context("No selected file")?)
            .await?;
        let last_modified = match metadata.last_modified() {
            Some(dt) => &dt.format("%Y/%m/%d %H:%M:%S").to_string(),
            None => "",
        };
        let size = human_size(metadata.content_length());
        let metadata_repr = format!("{size} {last_modified} {filename} ");
        self.metadata_repr = Some((self.index, metadata_repr));

        Ok(())
    }

    /// True if the opendal container is really set. IE if it's connected to a remote container.
    pub fn is_set(&self) -> bool {
        self.op.is_some()
    }

    fn cloud_build_dest_filename(&self, local_file: &FileInfo) -> String {
        let filename = local_file.filename.as_ref();
        let mut dest_path = self.path.clone();
        dest_path.push(filename);
        path_to_string(&dest_path)
    }

    /// Upload the local file to the remote container in its current path.
    #[tokio::main]
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

    fn selected_filename(&self) -> Option<&str> {
        self.selected()?.path().split('/').last()
    }

    fn create_downloaded_path(&self, dest: &std::path::Path) -> Option<std::path::PathBuf> {
        let distant_filename = self.selected_filename()?;
        let mut dest = dest.to_path_buf();
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

    /// Download the currently selected remote file to dest. The filename is preserved.
    /// Nothing is done if a local file with same filename already exists in current path.
    ///
    /// This will most likely change in the future since it's not the default behavior of
    /// most modern file managers.
    #[tokio::main]
    pub async fn download(&self, dest: &std::path::Path) -> Result<()> {
        let Some(op) = &self.op else {
            return Ok(());
        };
        let Some(selected) = self.selected() else {
            return Ok(());
        };
        let distant_filepath = selected.path();
        let Some(dest_full_path) = self.create_downloaded_path(dest) else {
            return Ok(());
        };
        let buf = op.read(distant_filepath).await?;
        let mut file = File::create(&dest_full_path).await?;
        file.write_all(&buf.to_bytes()).await?;
        log_info!(
            "Downloaded {distant_filepath} to local file {path}",
            path = dest_full_path.display(),
        );
        Ok(())
    }

    /// Creates a new remote directory with dirname in current path.
    #[tokio::main]
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

    /// Disconnect itself, reseting it's parameters.
    pub fn disconnect(&mut self) {
        let desc = self.name.to_owned();
        self.op = None;
        self.kind = OpendalKind::Empty;
        self.name = "empty".to_owned();
        self.path = std::path::PathBuf::from("");
        self.root = std::path::PathBuf::from("");
        self.index = 0;
        self.content = vec![];
        log_info!("Disconnected from {desc}");
    }

    /// Delete the currently selected remote file
    /// Nothing is done if current path is empty.
    #[tokio::main]
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

    async fn update_path(&mut self, path: &str) -> Result<()> {
        if let Some(op) = &self.op {
            self.content = op.list(path).await?;
            self.path = std::path::PathBuf::from(path);
            self.index = 0;
            self.metadata_repr = None;
        };
        Ok(())
    }

    /// Enter in the selected file or directory.
    ///
    /// # Errors:
    ///
    /// Will fail if the selected file is not a directory of the current path is empty.
    #[tokio::main]
    pub async fn enter_selected(&mut self) -> Result<()> {
        let path = self.selected().context("no path")?.path().to_owned();
        self.update_path(&path).await
    }

    fn ensure_index_in_bounds(&mut self) {
        self.index = std::cmp::min(self.content.len().saturating_sub(1), self.index)
    }

    /// Refresh the current remote path.
    /// Nothing is done if no connexion is established.
    #[tokio::main]
    pub async fn refresh_current(&mut self) -> Result<()> {
        let old_index = self.index;
        self.update_path(&path_to_string(&self.path)).await?;
        self.index = old_index;
        self.ensure_index_in_bounds();
        Ok(())
    }

    /// Move to remote parent directory if possible
    #[tokio::main]
    pub async fn move_to_parent(&mut self) -> Result<()> {
        if self.op.is_some() {
            if self.path == self.root {
                return Ok(());
            };
            if let Some(parent) = self.path.to_owned().parent() {
                self.update_path(&path_to_string(&parent)).await?;
            }
        }
        Ok(())
    }

    /// Format a description of the current container: Name and path.
    pub fn desc(&self) -> String {
        format!(
            "{desc}{sep}{path}",
            desc = self.name,
            sep = if self.path == self.root { "" } else { "/" },
            path = self.path.display()
        )
    }
}

impl_selectable!(OpendalContainer);
impl_content!(Entry, OpendalContainer);
