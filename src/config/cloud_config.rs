use anyhow::{Context, Result};
use oauth2::basic::{BasicClient, BasicTokenType};
use oauth2::reqwest::async_http_client;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EmptyExtraTokenFields,
    RedirectUrl, Scope, StandardTokenResponse, TokenResponse, TokenUrl,
};
use tokio::io::{self, AsyncBufReadExt, BufReader};

use crate::common::path_to_config_folder;

async fn read_input() -> String {
    let mut input = String::new();
    let mut stdin = BufReader::new(io::stdin());
    stdin
        .read_line(&mut input)
        .await
        .expect("Couldn't read input");
    input.trim().to_string()
}

async fn gather_input_data() -> (String, String, String, String) {
    println!("This application will create a refresh token allowing you to access your files on google drive from fm.
It will also create a token file used by fm.
You need to setup GoogleDrive's API from your account first.
Please refer to the fm documentation on GitHub for more information about it. : https://github.com/qkzk/fm

Please enter a friendly name for your google drive folder:");
    let drive_name = read_input().await;
    println!("Please enter your root folder. Default is /:");
    let mut root_folder = read_input().await;
    if root_folder.is_empty() {
        root_folder = "/".to_string();
    }

    println!("Please enter your google cloud client id:");
    let client_id = read_input().await;
    println!("Please enter your google cloud client secret:");
    let client_secret = read_input().await;
    (drive_name, root_folder, client_id, client_secret)
}

fn create_client(client_id: &str, client_secret: &str) -> Result<BasicClient> {
    let client = BasicClient::new(
        ClientId::new(client_id.to_string()),
        Some(ClientSecret::new(client_secret.to_string())),
        AuthUrl::new("https://accounts.google.com/o/oauth2/auth".to_string())?,
        Some(TokenUrl::new(
            "https://oauth2.googleapis.com/token".to_string(),
        )?),
    )
    // Set the URL the user will be redirected to after the authorization process.
    .set_redirect_uri(RedirectUrl::new("urn:ietf:wg:oauth:2.0:oob".to_string())?);
    Ok(client)
}

fn get_auth_url(client: &BasicClient) -> url::Url {
    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        // Set the desired scopes.
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/drive".to_string(),
        ))
        .add_extra_param("access_type", "offline") // Request offline access for a refresh
        .url();
    println!("token {csrf_token:?}", csrf_token = csrf_token.secret());
    auth_url
}

async fn get_token_result(
    client: &BasicClient,
    code: String,
) -> Result<StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>> {
    Ok(client
        .exchange_code(AuthorizationCode::new(code))
        .request_async(async_http_client)
        .await?)
}

fn extract_refresh_token(
    token_result: StandardTokenResponse<EmptyExtraTokenFields, BasicTokenType>,
) -> Result<String> {
    Ok(token_result
        .refresh_token()
        .context("Refresh token not provided")?
        .secret()
        .to_owned())
}

fn build_token_path(token_filename: &str) -> Result<std::path::PathBuf> {
    let mut token_path = path_to_config_folder()?;
    token_path.push(token_filename);
    Ok(token_path)
}

/// Creates a google drive token file for fm.
/// It will allow fm to list and manipulate the files on google drive.
#[tokio::main]
pub async fn cloud_config() -> Result<()> {
    // 1. Ask user a friendly name, a root folder, his id and secret.
    let (drive_name, root_folder, client_id, client_secret) = gather_input_data().await;

    // 2. Create an OAuth2 client by specifying the client ID, client secret, authorization URL, and token URL.
    let client = create_client(&client_id, &client_secret)?;

    // 3. Generate the authorization URL to which we'll redirect the user.
    let auth_url = get_auth_url(&client);

    // 4. Wait for the user to enter the authorization code.
    println!("Open this URL in your browser:\n{}\n", auth_url);
    println!("Enter the code you received after granting access:");
    let code = read_input().await;

    // 5. Exchange the authorization code with an access token.
    let token_result = get_token_result(&client, code).await?;

    // 6. Extract the refresh token from the response
    let refresh_token = extract_refresh_token(token_result)?;
    println!("Refresh token: {refresh_token}");

    // 7. Create the token filepath
    let token_filename = format!("token_{drive_name}.yaml");
    let token_path = build_token_path(&token_filename)?;

    // 8. Serialize the token
    let file_content = GoogleDriveConfig::serialized(
        drive_name,
        root_folder,
        refresh_token,
        client_id,
        client_secret,
    );

    // 8. Write the token file
    tokio::fs::write(&token_path, file_content.as_bytes()).await?;
    println!(
        "Token saved to {token_path}",
        token_path = token_path.display()
    );

    Ok(())
}

struct GoogleDriveConfig {
    drive_name: String,
    root_folder: String,
    refresh_token: String,
    client_id: String,
    client_secret: String,
}

impl GoogleDriveConfig {
    fn serialize(&self) -> String {
        format!(
            "drive_name: \"{dn}\"
root_folder: \"{rf}\"
refresh_token: \"{rt}\"
client_id: \"{ci}\"
client_secret: \"{cs}\"",
            dn = self.drive_name,
            rf = self.root_folder,
            rt = self.refresh_token,
            ci = self.client_id,
            cs = self.client_secret,
        )
    }

    fn serialized(
        drive_name: String,
        root_folder: String,
        refresh_token: String,
        client_id: String,
        client_secret: String,
    ) -> String {
        Self {
            drive_name,
            root_folder,
            refresh_token,
            client_id,
            client_secret,
        }
        .serialize()
    }
}
