use crate::{
    db,
    error::{Error, Result},
    twilio::send_sms,
    AppTemplate, DISCORD_AVATAR_URL,
};
use crate::{AppState, NewQuestionTemplate};
use askama::Template;
use axum::{
    extract::{Extension, Form, Path, State},
    response::Redirect,
};
use serde::{Deserialize, Serialize};

fn user_image_url(user: &db::User) -> String {
    let ext = match user.avatar.starts_with("a_") {
        true => "gif",
        false => "png",
    };
    format!(
        "{}/{}/{}.{}",
        DISCORD_AVATAR_URL,
        user.id,
        user.avatar.clone(),
        ext
    )
}

#[derive(Serialize, Deserialize)]
pub struct QuestionUser {
    pub question_id: String,
    pub title: String,
    pub body: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub public: bool,
    // Fields from User
    pub user_id: String,
    pub username: String,
    pub avatar: String,

    // aditional fields
    pub answered: bool,
    pub answer_body: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct QuestionAnswered {
    pub id: String,
    pub title: String,
    pub body: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub public: bool,
    pub answered: bool,
    pub answer_body: Option<String>,
}

pub async fn app(
    Extension(user): Extension<db::User>,
    State(state): State<AppState>,
) -> Result<AppTemplate> {
    let questions = if user.is_admin {
        state.db.get_unanswered_questions().await?
    } else {
        state
            .db
            .get_questions_by_user_id(&user.id)
            .await?
            .iter()
            .map(|q| QuestionUser {
                avatar: user.avatar.clone(),
                username: user.username.clone(),
                user_id: user.id.clone(),
                question_id: q.id.clone(),
                title: q.title.clone(),
                body: q.body.clone(),
                created_at: q.created_at,
                public: q.public,
                answered: q.answered,
                answer_body: q.answer_body.clone(),
            })
            .collect()
    };
    let daily_questions = state.db.get_user_daily_questions(&user.id).await?;

    let remaining = user.daily_questions - daily_questions.len() as u64;
    Ok(AppTemplate {
        image_url: user_image_url(&user),
        user_limit: user.daily_questions,
        user,
        q_count: questions.len(),
        questions,
        remaining,
    })
}

pub async fn new_question(Extension(user): Extension<db::User>) -> Result<NewQuestionTemplate> {
    Ok(NewQuestionTemplate {
        image_url: user_image_url(&user),
        user,
    })
}

#[derive(Debug, Deserialize)]
pub struct NewQuestionForm {
    title: String,
    body: Option<String>,
    public: bool,
}

pub async fn submit_question(
    State(state): State<AppState>,
    Extension(user): Extension<db::User>,
    Form(form): Form<NewQuestionForm>,
) -> Result<Redirect> {
    let body = form.body.unwrap_or_default();
    let public = form.public;
    let title = form.title;

    if title.len() < 5 || title.len() > 100 {
        return Err(Error::InvalidQuestionTitle);
    }

    if body.len() > 1000 {
        return Err(Error::InvalidQuestionBody);
    }

    let daily_limit = user.daily_questions;

    let questions = state.db.get_user_daily_questions(&user.id).await?;

    if (questions.len() as u64) < daily_limit {
        let question = db::Question {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.clone(),
            body,
            public,
            created_at: chrono::Utc::now(),
            user_id: user.id.clone(),
        };
        state.db.create_question(question).await?;

        let _ = send_sms(
            &state.http,
            &format!("Question submitted by {}: {}", user.username, title),
        )
        .await;
    }

    Ok(Redirect::to("/app"))
}

#[derive(Template)]
#[template(path = "app/new_answer.html")]
pub struct AppAnswerTemplate {
    user: db::User,
    question: db::Question,
    image_url: String,
}

pub async fn answer(
    Path(id): Path<String>,
    Extension(user): Extension<db::User>,
    State(state): State<AppState>,
) -> Result<AppAnswerTemplate> {
    if !user.is_admin {
        return Err(Error::Unauthorized);
    }

    let question = state
        .db
        .get_question(&id)
        .await?
        .ok_or(Error::QuestionNotFound)?;

    Ok(AppAnswerTemplate {
        question,
        image_url: user_image_url(&user),
        user,
    })
}

#[derive(Debug, Deserialize)]
pub struct NewAnswerForm {
    body: String,
}

pub async fn submit_answer(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Extension(user): Extension<db::User>,
    Form(form): Form<NewAnswerForm>,
) -> Result<Redirect> {
    let body = form.body.trim().to_string();

    if !user.is_admin {
        return Err(Error::Unauthorized);
    }

    let question = state
        .db
        .get_question(&id)
        .await?
        .ok_or(Error::QuestionNotFound)?;

    let answer = state.db.get_question_answer(&question.id).await?;

    if answer.is_some() {
        return Err(Error::AnswerAlreadyExists);
    }

    let answer = db::Answer {
        id: uuid::Uuid::new_v4().to_string(),
        body,
        created_at: chrono::Utc::now(),
        user_id: user.id.clone(),
        question_id: question.id.clone(),
    };
    state.db.create_answer(answer).await?;

    Ok(Redirect::to("/"))
}
