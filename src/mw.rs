use crate::{AppState, COOKIE_NAME};
use axum::{
    body::Body,
    extract::{ConnectInfo, Request, State},
    http::{header::LOCATION, StatusCode},
    middleware::Next,
    response::Response,
};
use axum_extra::{headers::Cookie, TypedHeader};
use governor::{
    clock::DefaultClock, state::keyed::DashMapStateStore, Quota, RateLimiter as GovernorRateLimiter,
};
use std::{
    net::SocketAddr,
    sync::Arc,
    task::{Context, Poll},
};
use tower::{Layer, Service};

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

#[derive(Debug, Clone)]
pub struct RateLimiterLayer {
    clients: Arc<GovernorRateLimiter<String, DashMapStateStore<String>, DefaultClock>>,
}

impl RateLimiterLayer {
    pub fn new(quota: Quota) -> Self {
        let clients = Arc::new(GovernorRateLimiter::keyed(quota));
        Self { clients }
    }
}

impl<S> Layer<S> for RateLimiterLayer {
    type Service = RateLimiter<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimiter {
            inner,
            layer: Arc::new(self.clone()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RateLimiter<S> {
    inner: S,
    layer: Arc<RateLimiterLayer>,
}

impl<S> Service<Request> for RateLimiter<S>
where
    S: Service<Request, Response = Response> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = futures::future::BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let client_ip = req.extensions().get::<ConnectInfo<SocketAddr>>().cloned();
        let fut = self.inner.call(req);
        let layer = self.layer.clone();

        Box::pin(async move {
            let client_ip = if let Some(ConnectInfo(addr)) = client_ip {
                addr.ip().to_string()
            } else {
                return Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from("Missing ConnectInfo"))
                    .unwrap());
            };
            match layer.clients.check_key(&client_ip) {
                Ok(()) => {
                    let res: Response = fut.await?;
                    Ok(res)
                }
                Err(_) => Ok(Response::builder()
                    .status(StatusCode::TOO_MANY_REQUESTS)
                    .body(Body::from("Too many requests"))
                    .unwrap()),
            }
        })
    }
}
