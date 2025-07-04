use anyhow::{bail, Context, Result};
use http_body_util::Full;
use hyper::{
    body::Bytes, body::Incoming, header, server::conn::http1, service::Service, Request, Response,
};
use hyper_util::rt::tokio::TokioIo;
use oauth2::{
    basic::BasicClient, reqwest, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use tokio::{
    io::{self, AsyncBufReadExt, BufReader},
    net::TcpListener,
    sync::Mutex,
    time::sleep,
};
use url::form_urlencoded;

use std::{
    convert::Infallible, future::Future, path::PathBuf, pin::Pin, sync::Arc, time::Duration,
};

use crate::common::path_to_config_folder;
use crate::io::GoogleDriveConfig;

async fn read_input() -> String {
    let mut input = String::new();
    let mut stdin = BufReader::new(io::stdin());
    stdin
        .read_line(&mut input)
        .await
        .expect("Couldn't read input");
    input.trim().to_string()
}

struct ConfigSetup {
    drive_name: String,
    root_folder: String,
    client_id: String,
    client_secret: String,
}

async fn gather_input_data() -> ConfigSetup {
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
    ConfigSetup {
        drive_name,
        root_folder,
        client_id,
        client_secret,
    }
}

type OauthClient = oauth2::Client<
    oauth2::StandardErrorResponse<oauth2::basic::BasicErrorResponseType>,
    oauth2::StandardTokenResponse<oauth2::EmptyExtraTokenFields, oauth2::basic::BasicTokenType>,
    oauth2::StandardTokenIntrospectionResponse<
        oauth2::EmptyExtraTokenFields,
        oauth2::basic::BasicTokenType,
    >,
    oauth2::StandardRevocableToken,
    oauth2::StandardErrorResponse<oauth2::RevocationErrorResponseType>,
    oauth2::EndpointSet,
    oauth2::EndpointNotSet,
    oauth2::EndpointNotSet,
    oauth2::EndpointNotSet,
    oauth2::EndpointSet,
>;

fn create_client(client_id: &str, client_secret: &str) -> Result<OauthClient> {
    let client = BasicClient::new(ClientId::new(client_id.to_string()))
        .set_client_secret(ClientSecret::new(client_secret.to_string()))
        .set_auth_uri(AuthUrl::new(
            "https://accounts.google.com/o/oauth2/auth".to_string(),
        )?)
        .set_token_uri(TokenUrl::new(
            "https://oauth2.googleapis.com/token".to_string(),
        )?)
        // Set the URL the user will be redirected to after the authorization process.
        .set_redirect_uri(RedirectUrl::new(format!(
            "http://localhost:{DEFAULT_PORT}"
        ))?);
    Ok(client)
}

fn get_auth_url(client: &OauthClient) -> url::Url {
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

type Stt =
    oauth2::StandardTokenResponse<oauth2::EmptyExtraTokenFields, oauth2::basic::BasicTokenType>;

async fn get_token_result(client: &OauthClient, code: String) -> Result<Stt> {
    let http_client = reqwest::ClientBuilder::new()
        // Following redirects opens the client up to SSRF vulnerabilities.
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build");
    match client
        .exchange_code(AuthorizationCode::new(code))
        .request_async(&http_client)
        .await
    {
        Ok(res) => Ok(res),
        Err(error) => {
            println!("Error: {error:?}");
            bail!("Error {error}")
        }
    }
}

fn extract_refresh_token(token_result: Stt) -> Result<String> {
    Ok(token_result
        .refresh_token()
        .context("Refresh token not provided")?
        .secret()
        .to_owned())
}

fn build_token_path(token_filename: &str) -> Result<PathBuf> {
    let mut token_path = path_to_config_folder()?;
    token_path.push(token_filename);
    Ok(token_path)
}

const DEFAULT_PORT: u16 = 44444;

async fn receive_code_from_localhost(port: u16) -> Result<String> {
    let listener = TcpListener::bind(("127.0.0.1", port)).await?;
    println!("Waiting for a code on http://localhost:{port}");

    let (stream, _) = listener.accept().await?;

    let code_slot = Arc::new(Mutex::new(None));
    let handler = Handler {
        code_slot: code_slot.clone(),
    };

    let io = TokioIo::new(stream);
    let conn = http1::Builder::new().serve_connection(io, handler);

    // serving the connexion in parallel
    tokio::spawn(conn);

    // waiting the code...
    loop {
        sleep(Duration::from_millis(100)).await;
        let mut slot = code_slot.lock().await;
        if let Some(code) = slot.take() {
            return Ok(code);
        }
    }
}

struct Handler {
    code_slot: Arc<Mutex<Option<String>>>,
}

impl Service<Request<Incoming>> for Handler {
    type Response = Response<Full<Bytes>>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let code_slot = self.code_slot.clone();

        Box::pin(async move {
            let code = read_code(req);
            let body = build_body(code, code_slot).await;
            let response = build_response(body);
            Ok(response)
        })
    }
}

fn read_code(req: Request<Incoming>) -> Option<String> {
    let query = req.uri().query().unwrap_or("");
    form_urlencoded::parse(query.as_bytes())
        .find(|(k, _)| k == "code")
        .map(|(_, v)| v.to_string())
}

async fn build_body(code: Option<String>, code_slot: Arc<Mutex<Option<String>>>) -> Full<Bytes> {
    if let Some(code) = code {
        *code_slot.lock().await = Some(code.clone());
        println!("received code: {code}");
        Full::from("SUCCESS! Code received properly. You can close this window.")
    } else {
        Full::from("FAILURE! No code received from URL. You can close this window.")
    }
}

fn build_response(body: Full<Bytes>) -> Response<Full<Bytes>> {
    let mut response = Response::new(body);
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        "text/html; charset=utf-8".parse().unwrap(),
    );
    response
}

/// Creates a google drive token file for fm.
/// It will allow fm to list and manipulate the files on google drive.
#[tokio::main]
pub async fn cloud_config() -> Result<()> {
    // 1. Ask user a friendly name, a root folder, his id and secret.
    let ConfigSetup {
        drive_name,
        root_folder,
        client_id,
        client_secret,
    } = gather_input_data().await;

    // 2. Create an OAuth2 client by specifying the client ID, client secret, authorization URL, and token URL.
    let client = create_client(&client_id, &client_secret)?;

    // 3. Generate the authorization URL where we'll redirect the user.
    let auth_url = get_auth_url(&client);

    // 4. Wait for the user to follow authorization from google cloud.
    println!("Open this URL in your browser:\n{}\n", auth_url);
    let code = receive_code_from_localhost(DEFAULT_PORT).await?;

    // 5. Exchange the authorization code with an access token.
    let token_result = get_token_result(&client, code).await?;

    // 6. Extract the refresh token from the response
    let refresh_token = extract_refresh_token(token_result)?;
    println!("Refresh token: {refresh_token}");

    // 7. Create the token filepath
    let token_filename = format!("token_{drive_name}.yaml");
    let token_path = build_token_path(&token_filename)?;

    // 8. Serialize the token
    let file_content = GoogleDriveConfig::new(
        drive_name.clone(),
        root_folder,
        refresh_token,
        client_id,
        client_secret,
    )
    .serialize()?;

    // 8. Write the token file
    tokio::fs::write(&token_path, file_content.as_bytes()).await?;
    println!(
        "Token saved to {token_path}",
        token_path = token_path.display()
    );
    println!("Everything is done, you should be able to access your drive {drive_name}");

    Ok(())
}
