use crate::{
    crypto::{self, get_key},
    db,
    error::{Error, Result},
    AppState, ADMIN_ID, COOKIE_NAME, GENERIC_DAILY_LIMIT,
};
use axum::{
    extract::{Query, State},
    http::header::{HeaderMap, SET_COOKIE},
    response::{IntoResponse, Redirect},
};
use oauth2::{reqwest::async_http_client, AuthorizationCode, Scope, TokenResponse};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct AuthRequest {
    code: String,
    state: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct DiscordUser {
    id: String,
    avatar: Option<String>,
    username: String,
    discriminator: String,
    email: Option<String>,
}

pub async fn discord_cb(
    Query(query): Query<AuthRequest>,
    state: State<AppState>,
) -> Result<impl IntoResponse> {
    let token = state
        .oauth
        .exchange_code(AuthorizationCode::new(query.code.clone()))
        .request_async(async_http_client)
        .await
        .map_err(|e| Error::Auth(e.to_string()))?;
    let user_data: DiscordUser = state
        .http
        .get("https://discordapp.com/api/users/@me")
        .bearer_auth(token.access_token().secret())
        .send()
        .await
        .map_err(|e| Error::Auth(e.to_string()))?
        .json::<DiscordUser>()
        .await
        .map_err(|e| Error::Auth(e.to_string()))?;

    let user = state.db.get_user(&user_data.id).await?;
    if user.is_none() {
        let is_admin = user_data.id == ADMIN_ID.to_string();
        let user = db::User {
            id: user_data.id.clone(),
            username: user_data.username.clone(),
            discriminator: user_data.discriminator.clone(),
            avatar: user_data.avatar.unwrap(),
            is_admin,
            daily_questions: GENERIC_DAILY_LIMIT,
            joined_at: chrono::Utc::now(),
            last_question_reset: None,
        };
        state.db.create_user(user).await?;
    }

    if let Some(existing_session) = state
        .db
        .get_active_session_by_user_id(&user_data.id)
        .await?
    {
        state.db.delete_session(&existing_session.id).await?;
    }
    let session_id = uuid::Uuid::new_v4();
    let expires_in = token.expires_in().map(|d| d.as_secs()).unwrap_or(3600); // Default to 1 hour
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(expires_in as i64);

    let cookie = format!(
        "{COOKIE_NAME}={cookie}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}",
        cookie = session_id,
        max_age = expires_at.timestamp() - chrono::Utc::now().timestamp()
    );

    let encryption_key = get_key();

    let access_token_plain = token.access_token().secret();
    let (access_token, access_token_nonce) = crypto::encrypt(&encryption_key, access_token_plain);
    assert_ne!(access_token, *access_token_plain);

    let refresh_token_plain = token.refresh_token().expect("refresh token").secret();
    let (refresh_token, refresh_token_nonce) =
        crypto::encrypt(&encryption_key, refresh_token_plain);
    assert_ne!(refresh_token, *refresh_token_plain);

    let session = db::Session {
        id: session_id.to_string(),
        user_id: user_data.id.clone(),
        access_token: access_token.clone(),
        access_token_nonce: access_token_nonce.clone(),
        refresh_token: refresh_token.clone(),
        refresh_token_nonce: refresh_token_nonce.clone(),
        expires_at,
    };
    state.db.create_session(session).await?;

    let mut headers = HeaderMap::new();
    headers.insert(SET_COOKIE, cookie.parse().expect("failed to parse cookie"));
    Ok((headers, Redirect::to("/app")))
}

pub async fn discord_auth(state: State<AppState>) -> impl IntoResponse {
    let (auth_url, _csrf_token) = state
        .oauth
        .authorize_url(oauth2::CsrfToken::new_random)
        .add_scopes([
            Scope::new("identify".to_string()),
            Scope::new("email".to_string()),
        ])
        .url();

    Redirect::to(auth_url.as_ref())
}
