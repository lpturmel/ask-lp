use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tracing::error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Auth(String),
    Config(String),
    Db(libsql::Error),
    UserAlreadyExists,
    InvalidQuestionTitle,
    InvalidQuestionBody,
    DailyLimitReached,
    QuestionNotFound,
    Unauthorized,
    AnswerAlreadyExists,
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        error!("Error: {:?}", self);
        let (status, error_message) = match self {
            Error::Auth(_) => (StatusCode::UNAUTHORIZED, "Unauthorized"),
            Error::AnswerAlreadyExists => (
                StatusCode::BAD_REQUEST,
                "Answer already exists for this question",
            ),
            Error::Config(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error"),
            Error::Db(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error"),
            Error::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized"),
            Error::InvalidQuestionTitle => (
                StatusCode::BAD_REQUEST,
                "Invalid Question Title. Title must be between 5 and 100 characters",
            ),
            Error::InvalidQuestionBody => (
                StatusCode::BAD_REQUEST,
                "Invalid Question Body. Body must be between 5 and 100 characters",
            ),
            Error::DailyLimitReached => (
                StatusCode::BAD_REQUEST,
                "Daily question limit reached. Come back tomorrow to submit another question",
            ),
            Error::QuestionNotFound => (StatusCode::NOT_FOUND, "Question not found"),
            Error::UserAlreadyExists => (StatusCode::BAD_REQUEST, "User Already Exists"),
        };

        if status == StatusCode::NOT_FOUND {
            let template = crate::NotFoundTemplate {
                message: error_message.to_string(),
            };
            template.into_response()
        } else {
            (status, error_message).into_response()
        }
    }
}

impl From<libsql::Error> for Error {
    fn from(e: libsql::Error) -> Self {
        Error::Db(e)
    }
}
