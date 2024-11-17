use crate::handlers::app::QuestionAnswered;
use crate::{auth::refresh_access_token, handlers::app::QuestionUser, ADMIN_ID};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use chrono_tz::America::{self};
use futures::StreamExt;
use libsql::{de::from_row, params, Connection};
use oauth2::basic::BasicClient;
use serde::{Deserialize, Serialize};
use tracing::error;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub id: String,
    pub username: String,
    pub discriminator: String,
    pub avatar: String,
    pub is_admin: bool,
    pub joined_at: chrono::DateTime<chrono::Utc>,
    pub daily_questions: u64,
    pub last_question_reset: Option<NaiveDate>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub access_token: String,
    pub access_token_nonce: String,
    pub refresh_token: String,
    pub refresh_token_nonce: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Question {
    pub id: String,
    pub title: String,
    pub body: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub public: bool,
    pub user_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Answer {
    pub id: String,
    pub body: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub user_id: String,
    pub question_id: String,
}

#[derive(Clone)]
pub struct Model {
    conn: Connection,
}

impl Model {
    pub fn new(conn: Connection) -> Self {
        Self { conn }
    }

    pub async fn get_active_session(
        &self,
        oauth_client: &BasicClient,
        id: &str,
    ) -> Result<Option<Session>, libsql::Error> {
        let param = params!(id);
        let mut rows = self
            .conn
            .query("SELECT * FROM Session WHERE id = ?", param)
            .await?;
        if let Some(row) = rows.next().await? {
            let session: Session =
                from_row::<Session>(&row).expect("Failed to deserialize row into Session type");

            if session.expires_at > chrono::Utc::now() {
                Ok(Some(session))
            } else {
                match refresh_access_token(oauth_client, &session).await {
                    Ok(new_session) => {
                        self.update_session(&new_session).await?;
                        Ok(Some(new_session))
                    }
                    Err(e) => {
                        error!("Failed to refresh access token: {:?}", e);
                        self.delete_session(&session.id).await?;
                        Ok(None)
                    }
                }
            }
        } else {
            Ok(None)
        }
    }
    pub async fn get_active_session_by_user_id(
        &self,
        user_id: &str,
    ) -> Result<Option<Session>, libsql::Error> {
        let param = params!(user_id);
        let mut rows = self
            .conn
            .query("SELECT * FROM Session WHERE user_id = ?", param)
            .await?;
        while let Some(row) = rows.next().await? {
            let session: Session =
                from_row::<Session>(&row).expect("Failed to deserialize row into Session type");
            if session.expires_at > chrono::Utc::now() {
                // Active session found
                return Ok(Some(session));
            } else {
                // Session expired; delete it
                self.delete_session(&session.id).await?;
            }
        }
        Ok(None)
    }
    pub async fn get_session(&self, id: &str) -> Result<Option<Session>, libsql::Error> {
        let param = params!(id);
        let mut rows = self
            .conn
            .query("SELECT * FROM Session WHERE id = ?", param)
            .await?;
        let first = rows.next().await?;
        Ok(first
            .map(|r| from_row::<Session>(&r).expect("Failed to deserialize row into Session type")))
    }
    pub async fn create_user(&self, user: User) -> Result<(), libsql::Error> {
        let last_question_reset = user.last_question_reset.map(|d| d.to_string());
        let params = params!(
            user.id,
            user.username,
            user.discriminator,
            user.avatar,
            user.is_admin,
            user.joined_at.to_rfc3339(),
            user.daily_questions,
            last_question_reset
        );
        self.conn
            .execute(
                "INSERT INTO User (id, username, discriminator, avatar, is_admin, joined_at, daily_questions, last_question_reset) VALUES (?,?,?,?,?,?,?,?)",
                params,
            )
            .await?;
        Ok(())
    }

    pub async fn get_user(&self, id: &str) -> Result<Option<User>, libsql::Error> {
        let param = params!(id);
        let mut rows = self
            .conn
            .query("SELECT * FROM User WHERE id = ?", param)
            .await?;
        let first = rows.next().await?;
        Ok(first.map(|r| from_row::<User>(&r).expect("Failed to deserialize row into User type")))
    }

    pub async fn get_users(&self) -> Result<Vec<User>, libsql::Error> {
        let res = self.conn.query("SELECT * FROM User", params![]).await?;
        let stream = res.into_stream();

        let users = stream
            .map(|row| from_row::<User>(&row.unwrap()).unwrap())
            .collect::<Vec<_>>()
            .await;
        Ok(users)
    }

    pub async fn create_session(&self, session: Session) -> Result<(), libsql::Error> {
        let params = params!(
            session.id,
            session.user_id,
            session.access_token,
            session.access_token_nonce,
            session.refresh_token,
            session.refresh_token_nonce,
            session.expires_at.to_rfc3339(),
        );
        self.conn
            .execute(
                "INSERT INTO Session (id, user_id, access_token, access_token_nonce, refresh_token, refresh_token_nonce, expires_at) VALUES (?,?,?,?,?,?,?)",
                params,
            )
            .await?;
        Ok(())
    }

    pub async fn delete_session(&self, id: &str) -> Result<(), libsql::Error> {
        let param = params!(id);
        self.conn
            .execute("DELETE FROM Session WHERE id = ?", param)
            .await?;
        Ok(())
    }

    pub async fn delete_sessions_by_user_id(&self, user_id: &str) -> Result<(), libsql::Error> {
        let param = params!(user_id);
        self.conn
            .execute("DELETE FROM Session WHERE user_id = ?", param)
            .await?;
        Ok(())
    }

    pub async fn get_questions_by_user_id(
        &self,
        user_id: &str,
    ) -> Result<Vec<QuestionAnswered>, libsql::Error> {
        let param = params!(user_id);
        let res = self
            .conn
            .query(
                "SELECT
                    Question.*,
                    EXISTS (
                        SELECT 1
                        FROM Answer
                        WHERE Answer.question_id = Question.id
                    ) as answered,
                    Answer.body as answer_body
                FROM Question
                LEFT JOIN Answer ON Answer.question_id = Question.id
                WHERE Question.user_id = ?
                ORDER BY created_at DESC
                ",
                param,
            )
            .await?;
        let stream = res.into_stream();

        let questions = stream
            .map(|row| from_row::<QuestionAnswered>(&row.unwrap()).unwrap())
            .collect::<Vec<_>>()
            .await;
        Ok(questions)
    }

    pub async fn create_question(&self, question: Question) -> Result<(), libsql::Error> {
        let params = params!(
            question.id,
            question.title,
            question.body,
            question.public,
            question.created_at.to_rfc3339(),
            question.user_id
        );
        self.conn
            .execute(
                "INSERT INTO Question (id, title, body, public, created_at, user_id) VALUES (?,?,?,?,?,?)",
                params,
            )
            .await?;
        Ok(())
    }
    pub async fn get_question(&self, id: &str) -> Result<Option<Question>, libsql::Error> {
        let param = params!(id);
        let mut rows = self
            .conn
            .query("SELECT * FROM Question WHERE id = ?", param)
            .await?;
        let first = rows.next().await?;
        Ok(first.map(|r| {
            from_row::<Question>(&r).expect("Failed to deserialize row into Question type")
        }))
    }

    pub async fn get_unanswered_questions(&self) -> Result<Vec<QuestionUser>, libsql::Error> {
        let param = params!(ADMIN_ID);
        let res = self
            .conn
            .query(
                "
                SELECT
                    Question.id AS question_id,
                    Question.title,
                    Question.body,
                    Question.created_at,
                    Question.public,
                    User.id AS user_id,
                    User.avatar,
                    User.username,
                    EXISTS (
                        SELECT 1
                        FROM Answer
                        WHERE Answer.question_id = Question.id
                    ) as answered

                FROM Question
                INNER JOIN User ON User.id = Question.user_id
                WHERE Question.user_id != ?
                  AND NOT EXISTS (
                    SELECT 1
                    FROM Answer
                    WHERE Answer.question_id = Question.id
                )    
                    ORDER BY Question.created_at DESC

                  ",
                param,
            )
            .await?;
        let stream = res.into_stream();

        let questions = stream
            .map(|row| from_row::<QuestionUser>(&row.unwrap()).unwrap())
            .collect::<Vec<_>>()
            .await;
        Ok(questions)
    }

    pub async fn get_user_daily_questions(
        &self,
        user_id: &str,
    ) -> Result<Vec<Question>, libsql::Error> {
        let now_utc: DateTime<Utc> = Utc::now();
        let now_eastern = now_utc.with_timezone(&America::New_York);

        let today_eastern = now_eastern.date_naive();
        let start_of_day_eastern = today_eastern
            .and_hms_opt(0, 0, 0)
            .expect("Failed to get start of day");
        let end_of_day_eastern = today_eastern
            .and_hms_opt(23, 59, 59)
            .expect("Failed to get end of day");

        let start_of_day_eastern_dt = America::New_York
            .from_local_datetime(&start_of_day_eastern)
            .unwrap();
        let end_of_day_eastern_dt = America::New_York
            .from_local_datetime(&end_of_day_eastern)
            .unwrap();

        let start_of_day_utc = start_of_day_eastern_dt.with_timezone(&Utc);
        let end_of_day_utc = end_of_day_eastern_dt.with_timezone(&Utc);

        let start_utc = start_of_day_utc.to_rfc3339();
        let end_utc = end_of_day_utc.to_rfc3339();

        let param = params!(user_id, start_utc, end_utc);
        let res = self
            .conn
            .query(
                "SELECT * FROM Question WHERE user_id = ? AND created_at BETWEEN ? AND ?",
                param,
            )
            .await?;
        let stream = res.into_stream();

        let questions = stream
            .map(|row| from_row::<Question>(&row.unwrap()).unwrap())
            .collect::<Vec<_>>()
            .await;
        Ok(questions)
    }

    pub async fn clean_up_expired_sessions(&self) -> Result<u64, libsql::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        let affected = self
            .conn
            .execute("DELETE FROM Session WHERE expires_at <= ?", params!(now))
            .await?;
        Ok(affected)
    }

    pub async fn update_session(&self, new_session: &Session) -> Result<(), libsql::Error> {
        let params = params!(
            new_session.access_token.clone(),
            new_session.refresh_token.clone(),
            new_session.expires_at.to_rfc3339(),
            new_session.id.clone(),
        );
        self.conn
        .execute(
            "UPDATE Session SET access_token = ?, refresh_token = ?, expires_at = ? WHERE id = ?",
            params,
        )
        .await?;
        Ok(())
    }

    pub async fn create_answer(&self, answer: Answer) -> Result<(), libsql::Error> {
        let params = params!(
            answer.id,
            answer.body,
            answer.created_at.to_rfc3339(),
            answer.user_id,
            answer.question_id
        );
        self.conn
            .execute(
                "INSERT INTO Answer (id, body, created_at, user_id, question_id) VALUES (?,?,?,?,?)",
                params,
            )
            .await?;
        Ok(())
    }
    pub async fn get_question_answer(
        &self,
        question_id: &str,
    ) -> Result<Option<Answer>, libsql::Error> {
        let param = params!(question_id);
        let mut rows = self
            .conn
            .query("SELECT * FROM Answer WHERE question_id = ?", param)
            .await?;
        let first = rows.next().await?;
        Ok(first
            .map(|r| from_row::<Answer>(&r).expect("Failed to deserialize row into Answer type")))
    }
}
