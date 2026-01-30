use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::Rng;
use sha2::{Digest, Sha256};

const SCOPES: &str = "https://www.googleapis.com/auth/calendar.readonly https://www.googleapis.com/auth/tasks";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub installed: InstalledCredentials,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledCredentials {
    pub client_id: String,
    pub client_secret: String,
    pub auth_uri: String,
    pub token_uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

fn get_base_dir() -> PathBuf {
    let exe_path = std::env::current_exe().unwrap_or_default();
    exe_path.parent().unwrap_or(&exe_path).to_path_buf()
}

fn get_credentials_path() -> PathBuf {
    get_base_dir().join("credentials.json")
}

fn get_token_path() -> PathBuf {
    get_base_dir().join("token.json")
}

pub fn load_credentials() -> Result<Credentials, String> {
    let path = get_credentials_path();
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read credentials.json: {}", e))?;
    serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse credentials.json: {}", e))
}

pub fn load_token() -> Option<Token> {
    let path = get_token_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            return serde_json::from_str(&content).ok();
        }
    }
    None
}

fn save_token(token: &Token) -> Result<(), String> {
    let path = get_token_path();
    let content = serde_json::to_string_pretty(token)
        .map_err(|e| format!("Failed to serialize token: {}", e))?;
    fs::write(&path, content)
        .map_err(|e| format!("Failed to write token.json: {}", e))
}

fn generate_code_verifier() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    URL_SAFE_NO_PAD.encode(&bytes)
}

fn generate_code_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let result = hasher.finalize();
    URL_SAFE_NO_PAD.encode(&result)
}

pub async fn get_access_token() -> Result<String, String> {
    let creds = load_credentials()?;

    // Check if we have a valid token
    if let Some(mut token) = load_token() {
        let now = chrono::Utc::now().timestamp();

        // Token still valid (with 60 second buffer)
        if let Some(expires_at) = token.expires_at {
            if expires_at > now + 60 {
                return Ok(token.access_token);
            }
        }

        // Try to refresh
        if let Some(ref refresh_token) = token.refresh_token {
            match refresh_access_token(&creds, refresh_token).await {
                Ok(new_token) => {
                    save_token(&new_token)?;
                    return Ok(new_token.access_token);
                }
                Err(e) => {
                    eprintln!("Failed to refresh token: {}", e);
                    // Fall through to re-auth
                }
            }
        }
    }

    // Need to do full OAuth flow
    let token = perform_oauth_flow(&creds).await?;
    save_token(&token)?;
    Ok(token.access_token)
}

async fn refresh_access_token(creds: &Credentials, refresh_token: &str) -> Result<Token, String> {
    let client = reqwest::Client::new();

    let params = [
        ("client_id", creds.installed.client_id.as_str()),
        ("client_secret", creds.installed.client_secret.as_str()),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
    ];

    let response = client
        .post(&creds.installed.token_uri)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Failed to refresh token: {}", e))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Token refresh failed: {}", error_text));
    }

    let token_response: TokenResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse token response: {}", e))?;

    let expires_at = token_response.expires_in.map(|secs| {
        chrono::Utc::now().timestamp() + secs
    });

    Ok(Token {
        access_token: token_response.access_token,
        refresh_token: token_response.refresh_token.or(Some(refresh_token.to_string())),
        expires_at,
    })
}

async fn perform_oauth_flow(creds: &Credentials) -> Result<Token, String> {
    let code_verifier = generate_code_verifier();
    let code_challenge = generate_code_challenge(&code_verifier);

    // Start local server to receive callback
    let server = tiny_http::Server::http("127.0.0.1:0")
        .map_err(|e| format!("Failed to start local server: {}", e))?;

    let port = server.server_addr().to_ip().unwrap().port();
    let redirect_uri = format!("http://127.0.0.1:{}", port);

    // Build auth URL
    let auth_url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={}&code_challenge_method=S256&access_type=offline&prompt=consent",
        creds.installed.auth_uri,
        urlencoding::encode(&creds.installed.client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(SCOPES),
        urlencoding::encode(&code_challenge)
    );

    // Open browser
    if let Err(e) = open::that(&auth_url) {
        eprintln!("Failed to open browser: {}. Please open this URL manually:\n{}", e, auth_url);
    }

    // Wait for callback
    let request = server.recv()
        .map_err(|e| format!("Failed to receive OAuth callback: {}", e))?;

    // Parse authorization code from URL
    let url_str = format!("http://localhost{}", request.url());
    let url = url::Url::parse(&url_str)
        .map_err(|e| format!("Failed to parse callback URL: {}", e))?;

    let code = url.query_pairs()
        .find(|(key, _)| key == "code")
        .map(|(_, value)| value.to_string())
        .ok_or_else(|| "No authorization code in callback".to_string())?;

    // Send response to browser
    let response_html = r#"
        <html>
        <body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; background: #1a1a2e;">
            <div style="text-align: center; color: white;">
                <h1>Authorization successful!</h1>
                <p>You can close this window.</p>
            </div>
        </body>
        </html>
    "#;

    let response = tiny_http::Response::from_string(response_html)
        .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html"[..]).unwrap());
    let _ = request.respond(response);

    // Exchange code for token
    let client = reqwest::Client::new();

    let params = [
        ("client_id", creds.installed.client_id.as_str()),
        ("client_secret", creds.installed.client_secret.as_str()),
        ("code", code.as_str()),
        ("code_verifier", code_verifier.as_str()),
        ("redirect_uri", redirect_uri.as_str()),
        ("grant_type", "authorization_code"),
    ];

    let response = client
        .post(&creds.installed.token_uri)
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Failed to exchange code for token: {}", e))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Token exchange failed: {}", error_text));
    }

    let token_response: TokenResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse token response: {}", e))?;

    let expires_at = token_response.expires_in.map(|secs| {
        chrono::Utc::now().timestamp() + secs
    });

    Ok(Token {
        access_token: token_response.access_token,
        refresh_token: token_response.refresh_token,
        expires_at,
    })
}
