use crate::{error::Result, AppState, COOKIE_NAME};
use axum::{
    extract::State,
    http::{header::SET_COOKIE, HeaderMap},
    response::IntoResponse,
    response::Redirect,
};
use axum_extra::{extract::TypedHeader, headers::Cookie};

pub mod app;
pub mod discord;
pub mod questions;

pub async fn logout(
    TypedHeader(cookies): TypedHeader<Cookie>,
    state: State<AppState>,
) -> Result<impl IntoResponse> {
    let cookie = cookies.get(COOKIE_NAME);
    let session_id = match cookie {
        Some(cookie) => cookie.to_string(),
        None => return Ok((HeaderMap::new(), Redirect::to("/"))),
    };
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        format!(
            "{COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
            COOKIE_NAME = COOKIE_NAME
        )
        .parse()
        .expect("failed to parse cookie"),
    );
    let session = match state.db.get_session(&session_id).await? {
        Some(s) => s,
        None => return Ok((headers, Redirect::to("/"))),
    };
    state.db.delete_session(&session.id).await?;

    Ok((headers, Redirect::to("/")))
}
