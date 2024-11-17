#![allow(dead_code)]
use askama::Template;
use axum::{extract::State, middleware, response::IntoResponse, routing, Json, Router};
use bot::Handler;
use error::Result;
use handlers::app::QuestionUser;
use libsql::Builder;
use oauth2::basic::BasicClient;
use serenity::all::{ActivityData, OnlineStatus};
use serenity::{all::GatewayIntents, Client};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;
use tracing::{error, info};

mod auth;
mod bot;
mod crypto;
mod db;
mod error;
mod handlers;
mod mw;
mod oai;
mod time;

pub const COOKIE_NAME: &str = "asklp_session";
pub const ADMIN_ID: u64 = 173963703606181888;
pub const DISCORD_AVATAR_URL: &str = "https://cdn.discordapp.com/avatars";
pub const GENERIC_DAILY_LIMIT: u64 = 10;

#[derive(Clone)]
pub struct AppState {
    db: db::Model,
    oauth: BasicClient,
    http: reqwest::Client,
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    #[cfg(debug_assertions)]
    let port = match std::env::var("PORT") {
        Ok(port) => port,
        Err(_) => "3000".to_string(),
    };
    #[cfg(not(debug_assertions))]
    let port = "8080";
    let port = port.parse::<u16>().unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    info!("Listening on {}", addr);

    // Set up database
    let url = std::env::var("LIBSQL_URL").unwrap();
    let token = std::env::var("LIBSQL_TOKEN").unwrap();
    let db = Builder::new_remote_replica("local.db", url, token)
        .sync_interval(Duration::from_secs(60))
        .build()
        .await
        .unwrap();

    db.sync().await.unwrap();

    let conn = db.connect().expect("Failed to connect to database");

    // Set up Discord bot
    let token = std::env::var("DISCORD_BOT_TOKEN").expect("Expected a token in the environment");
    let oai = oai::Client::new(&std::env::var("OPENAI_API_KEY").unwrap());
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let activity = ActivityData::watching("nbols");
    let mut client = Client::builder(&token, intents)
        .status(OnlineStatus::DoNotDisturb)
        .activity(activity)
        .event_handler(Handler::new(oai))
        .await
        .expect("Err creating client");

    tokio::spawn(async move {
        // Start listening for events by starting a single shard
        match client.start().await {
            Ok(_) => info!("Discord bot listening..."),
            Err(why) => error!("Client error: {why:?}"),
        }
    });

    let state = AppState {
        db: db::Model::new(conn),
        oauth: auth::oauth_client().unwrap(),
        http: reqwest::Client::new(),
    };

    let app_router = Router::new()
        .route("/", routing::get(handlers::app::app))
        .route("/question/:id/answer", routing::get(handlers::app::answer))
        .route(
            "/question/:id/answer/submit",
            routing::post(handlers::app::submit_answer),
        )
        .route("/question/new", routing::get(handlers::app::new_question))
        .route(
            "/question/submit",
            routing::post(handlers::app::submit_question),
        )
        .with_state(state.clone());

    let static_router = Router::new()
        .nest_service("/", ServeDir::new("static"))
        .layer(CompressionLayer::new());

    let app = Router::new()
        .route("/ping", routing::get(ping))
        .route("/", routing::get(index))
        .route("/logout", routing::get(handlers::logout))
        .route("/users", routing::get(get_users))
        .route(
            "/discord/callback",
            routing::get(handlers::discord::discord_cb),
        )
        .route(
            "/auth/discord",
            routing::get(handlers::discord::discord_auth),
        )
        // .route("/questions", routing::get(handlers::questions::questions))
        .nest("/app", app_router)
        .nest("/static", static_router)
        .fallback(not_found)
        .layer(middleware::from_fn_with_state(state.clone(), mw::auth))
        .with_state(state.clone());

    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn ping() -> &'static str {
    "pong"
}

async fn not_found() -> Result<NotFoundTemplate> {
    Ok(NotFoundTemplate {
        message: "Page not found".to_string(),
    })
}

async fn get_users(state: State<AppState>) -> Result<impl IntoResponse> {
    let users = state.db.get_users().await?;
    Ok(Json(users))
}

async fn index() -> Result<IndexTemplate> {
    Ok(IndexTemplate {
        login_url: "/auth/discord".to_string(),
    })
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    login_url: String,
}

#[derive(Template)]
#[template(path = "app/app.html")]
pub struct AppTemplate {
    user: db::User,
    image_url: String,
    questions: Vec<QuestionUser>,
    q_count: usize,
    remaining: u64,
    user_limit: u64,
}

#[derive(Template)]
#[template(path = "app/new_question.html")]
pub struct NewQuestionTemplate {
    image_url: String,
    user: db::User,
}

#[derive(Template)]
#[template(path = "notfound.html")]
struct NotFoundTemplate {
    message: String,
}
