use crate::{oai, ADMIN_ID};
use serenity::all::{Context, EventHandler, Message};
use serenity::async_trait;
use tracing::info;

#[derive(Clone)]
pub struct Handler {
    oai: oai::Client,
}

impl Handler {
    pub fn new(oai: oai::Client) -> Self {
        Self { oai }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        let serialized = serde_json::to_string(&msg).unwrap();

        let admin_mentioned = msg.mentions.iter().any(|m| m.id == ADMIN_ID);

        if !admin_mentioned {
            return;
        }

        let res = self
            .oai
            .create_chat_completion(oai::Model::Gpt4oMini, serialized)
            .await;

        info!(
            "Usage: input {} output {} total {} tokens",
            res.as_ref().map(|r| r.usage.prompt_tokens).unwrap_or(0),
            res.as_ref().map(|r| r.usage.completion_tokens).unwrap_or(0),
            res.as_ref().map(|r| r.usage.total_tokens).unwrap_or(0)
        );

        let is_question = res
            .ok()
            .and_then(|res| res.choices.first().cloned())
            .and_then(|c| c.message.content.to_lowercase().parse::<bool>().ok())
            .unwrap_or(false);

        let user = msg.author.clone();

        if !is_question {
            info!("User did not ask a question, ignoring message");
            return;
        }
        let reply = format!("<@{}> [ask lp](https://ask-lp.com)", user.id);
        info!(
            "Found a question from user {}",
            user.global_name.unwrap_or(user.name)
        );
        info!("Replying to user with link to ask-lp: {}", reply);
        msg.channel_id.say(ctx, reply).await.ok();
    }
}
