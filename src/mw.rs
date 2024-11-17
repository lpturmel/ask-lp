use crate::{AppState, COOKIE_NAME};
use axum::extract::{Request, State};
use axum::http::header::LOCATION;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use axum_extra::headers::Cookie;
use axum_extra::TypedHeader;

pub fn redirect_to(location: &str) -> Response {
    Response::builder()
        .status(StatusCode::TEMPORARY_REDIRECT)
        .header(LOCATION, location)
        .body(axum::body::Body::empty())
        .unwrap()
}

/// Middleware for protected routes
pub async fn auth(
    uri: axum::http::Uri,
    TypedHeader(cookies): TypedHeader<Cookie>,
    state: State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let uri_path = uri.path();
    let session_id = cookies.get(COOKIE_NAME);

    let user = if let Some(session_id) = session_id {
        if let Some(session) = state
            .db
            .get_active_session(&state.oauth, session_id)
            .await
            .ok()
            .flatten()
        {
            state.db.get_user(&session.user_id).await.ok().flatten()
        } else {
            None
        }
    } else {
        None
    };

    if let Some(user) = user {
        if uri_path == "/" {
            Ok(redirect_to("/app"))
        } else {
            req.extensions_mut().insert(user);
            Ok(next.run(req).await)
        }
    } else if uri_path.starts_with("/app") {
        Ok(redirect_to("/"))
    } else {
        Ok(next.run(req).await)
    }
}
