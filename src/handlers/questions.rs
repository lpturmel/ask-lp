// use crate::{db, error::Result, AppState};
// use axum::extract::State;

struct QueryString {
    user_id: Option<String>,
    answered: Option<bool>,
}

pub struct PublicQuestionSearch {
    id: String,
    title: String,
    body: String,
    created_at: chrono::DateTime<chrono::Utc>,
    user_id: String,
    username: String,
    answered: bool,
}
// pub async fn get_public_questions(
//     State(state): State<AppState>,
//     ) -> Result<PublicQuestionsTemplate> {
//
// }
