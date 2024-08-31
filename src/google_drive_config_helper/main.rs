use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::TokenResponse;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope, TokenUrl,
};
use tokio::io::{self, AsyncBufReadExt, BufReader};

async fn read_input() -> String {
    let mut input = String::new();
    let mut stdin = BufReader::new(io::stdin());
    stdin
        .read_line(&mut input)
        .await
        .expect("Couldn't read input");
    input.trim().to_string()
}

/// Creates a google drive token file for fm.
/// It will allow fm to list and manipulate the files on google drive.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("This application will create a refresh token allowing you to access your files on google drive. It will also create a token file used by fm.");
    println!("Please enter friendly name for your google drive folder:");
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
    // 1. Create an OAuth2 client by specifying the client ID, client secret, authorization URL, and token URL.
    let client = BasicClient::new(
        ClientId::new(client_id.clone()),
        Some(ClientSecret::new(client_secret.clone())),
        AuthUrl::new("https://accounts.google.com/o/oauth2/auth".to_string())?,
        Some(TokenUrl::new(
            "https://oauth2.googleapis.com/token".to_string(),
        )?),
    )
    // Set the URL the user will be redirected to after the authorization process.
    .set_redirect_uri(RedirectUrl::new("urn:ietf:wg:oauth:2.0:oob".to_string())?);

    // 2. Generate the authorization URL to which we'll redirect the user.
    let (auth_url, _) = client
        .authorize_url(CsrfToken::new_random)
        // Set the desired scopes.
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/drive".to_string(),
        ))
        .add_extra_param("access_type", "offline") // Request offline access for a refresh
        .url();

    println!("Open this URL in your browser:\n{}\n", auth_url);
    println!("Enter the code you received after granting access:");

    // 3. Wait for the user to enter the authorization code.
    let code = read_input().await;

    // 4. Exchange the authorization code with an access token.
    let token_result = client
        .exchange_code(AuthorizationCode::new(code))
        .request_async(async_http_client)
        .await?;

    let refresh_token = token_result.refresh_token().unwrap().secret().to_owned();
    println!("Refresh token: {refresh_token}");

    let google_drive_config = GoogleDriveConfig {
        drive_name,
        root_folder,
        client_id,
        client_secret,
        refresh_token,
    };
    let file_content = google_drive_config.serialize();
    let token_filename: String = "google_drive_token.yaml".to_string();
    tokio::fs::write(&token_filename, file_content.as_bytes())
        .await
        .expect("Couldn't write the token file");

    println!("Token saved to {token_filename}");
    Ok(())
}

#[derive(Debug)]
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
}
