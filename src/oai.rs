use crate::ADMIN_ID;
use reqwest::header::{HeaderMap, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fmt::Display;
use tracing::error;

#[derive(Debug, Clone)]
pub struct Client {
    inner: reqwest::Client,
    base_url: String,
}

#[derive(Debug)]
pub enum Error {
    Http,
    Json,
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        error!("Error: {:?}", e);
        Error::Http
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        error!("Error: {:?}", e);
        Error::Json
    }
}

impl Client {
    /// Create a new client with the given api key
    pub fn new(api_key: &str) -> Self {
        let c = reqwest::Client::builder();
        let mut default_headers = HeaderMap::new();

        default_headers.insert(
            AUTHORIZATION,
            format!("Bearer {}", api_key).parse().unwrap(),
        );

        let c = c.default_headers(default_headers).build().unwrap();

        Self {
            inner: c,
            base_url: "https://api.openai.com/v1".to_string(),
        }
    }
    /// Create a new chat completion request
    pub async fn create_chat_completion(
        &self,
        model: Model,
        content: String,
    ) -> Result<ChatCompletionResponse, Error> {
        let app_ctx =
            format!("your role is to analyze the context of a discord message to figure out if the user is asking a question to the user {}. Only answer with true or false. Messages might be in French or English, or a mix.", ADMIN_ID);
        let url = format!("{}/chat/completions", self.base_url);
        let messages = vec![
            (GptRole::System, app_ctx).into_gpt_message(),
            (GptRole::User, content).into_gpt_message(),
        ];
        let body = json!({
            "messages": messages,
            "max_tokens": 100,
            "temperature": 0,
            "model": model.to_string()
        });

        let res = self.inner.post(&url).json(&body).send().await?;

        let res: ChatCompletionResponse = res.json().await?;
        Ok(res)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GptMessage {
    pub role: GptRole,
    pub content: String,
}

/// Trait for converting a type into a GptMessage, used for chat completion requests to GPT-3.5
pub trait IntoGptMessage {
    fn into_gpt_message(self) -> GptMessage;
}

impl IntoGptMessage for GptMessage {
    fn into_gpt_message(self) -> GptMessage {
        self
    }
}

impl IntoGptMessage for (GptRole, String) {
    fn into_gpt_message(self) -> GptMessage {
        GptMessage {
            role: self.0,
            content: self.1,
        }
    }
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum GptRole {
    User,
    System,
    Assistant,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    pub index: i64,
    pub message: Message,
    #[serde(rename = "finish_reason")]
    pub finish_reason: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    #[serde(rename = "prompt_tokens")]
    pub prompt_tokens: i64,
    #[serde(rename = "completion_tokens")]
    pub completion_tokens: i64,
    #[serde(rename = "total_tokens")]
    pub total_tokens: i64,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Model {
    Gpt4o,
    Gpt4oMini,
}

impl Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Model::Gpt4o => write!(f, "gpt-4o-2024-08-06"),
            Model::Gpt4oMini => write!(f, "gpt-4o-mini-2024-07-18"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_validate_no_question() {
        let key = std::env::var("OPENAI_API_KEY").unwrap();
        let message = std::fs::read_to_string("tests/sample-mention-no-q.json").unwrap();
        let client = Client::new(&key);
        let res = client
            .create_chat_completion(Model::Gpt4oMini, message)
            .await
            .unwrap();

        let res = res.choices.first().unwrap();
        let res = res.message.content.to_lowercase().parse::<bool>();

        assert!(res.is_ok());
        assert!(!res.unwrap());
    }
    #[tokio::test]
    async fn test_validate_question() {
        let key = std::env::var("OPENAI_API_KEY").unwrap();
        let message = std::fs::read_to_string("tests/sample-mention-q.json").unwrap();
        let client = Client::new(&key);
        let res = client
            .create_chat_completion(Model::Gpt4oMini, message)
            .await
            .unwrap();

        let res = res.choices.first().unwrap();
        let res = res.message.content.to_lowercase().parse::<bool>();

        assert!(res.is_ok());
        assert!(res.unwrap());
    }
}
