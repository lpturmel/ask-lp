use crate::crypto::decrypt;
use crate::db::Session;
use crate::error::{Error, Result};
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use oauth2::{RefreshToken, TokenResponse};

pub fn oauth_client() -> Result<BasicClient> {
    let client_id = std::env::var("DISCORD_CLIENT_ID")
        .map_err(|_| Error::Config("Failed to get DISCORD_CLIENT_ID from env".to_string()))?;
    let client_secret = std::env::var("DISCORD_CLIENT_SECRET")
        .map_err(|_| Error::Config("Failed to get DISCORD_CLIENT_SECRET from env".to_string()))?;
    let redirect_uri = std::env::var("DISCORD_REDIRECT_URI")
        .map_err(|_| Error::Config("Failed to get DISCORD_REDIRECT_URI from env".to_string()))?;

    Ok(BasicClient::new(
        oauth2::ClientId::new(client_id),
        Some(oauth2::ClientSecret::new(client_secret)),
        oauth2::AuthUrl::new(
            "https://discord.com/api/oauth2/authorize?response_type=code".to_string(),
        )
        .unwrap(),
        Some(oauth2::TokenUrl::new("https://discord.com/api/oauth2/token".to_string()).unwrap()),
    )
    .set_redirect_uri(oauth2::RedirectUrl::new(redirect_uri).unwrap()))
}

pub async fn refresh_access_token(client: &BasicClient, session: &Session) -> Result<Session> {
    let encryption_key = crate::crypto::get_key();
    let refresh_token_plain = decrypt(
        &encryption_key,
        &session.refresh_token,
        &session.refresh_token_nonce,
    );
    let refresh_token = RefreshToken::new(refresh_token_plain);

    // Exchange the refresh token for a new access token
    let token_result = client
        .exchange_refresh_token(&refresh_token)
        .request_async(async_http_client)
        .await;

    match token_result {
        Ok(token) => {
            // Calculate new expiration time
            let expires_in = token.expires_in().map(|d| d.as_secs()).unwrap_or(3600);
            let expires_at = chrono::Utc::now() + chrono::Duration::seconds(expires_in as i64);

            let encryption_key = crate::crypto::get_key();
            let access_token_plain = token.access_token().secret();
            let (access_token, access_token_nonce) =
                crate::crypto::encrypt(&encryption_key, access_token_plain);
            assert_ne!(access_token, *access_token_plain);

            let refresh_token_plain = token.refresh_token().expect("refresh token").secret();
            let (refresh_token, refresh_token_nonce) =
                crate::crypto::encrypt(&encryption_key, refresh_token_plain);

            let new_session = Session {
                id: session.id.clone(),
                user_id: session.user_id.clone(),
                access_token: access_token.clone(),
                access_token_nonce: access_token_nonce.clone(),
                refresh_token: refresh_token.clone(),
                refresh_token_nonce: refresh_token_nonce.clone(),
                expires_at,
            };

            Ok(new_session)
        }
        Err(err) => {
            // Handle error (e.g., log it)
            Err(Error::Auth(format!("Token refresh failed: {}", err)))
        }
    }
}
