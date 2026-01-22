use aws_sdk_bedrockruntime::types::{ConversationRole, Message as BedrockMessage};
use serde::{Deserialize, Serialize};

use crate::content::{AssistantContent, UserContent};

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum Message {
    User { content: Vec<UserContent> },
    Assistant { content: Vec<AssistantContent> },
}

impl TryFrom<&Message> for BedrockMessage {
    type Error = anyhow::Error;

    fn try_from(message: &Message) -> Result<Self, Self::Error> {
        match message {
            Message::User { content } => {
                let content = content
                    .iter()
                    .map(Vec::try_from)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flatten()
                    .collect();

                Ok(BedrockMessage::builder()
                    .role(ConversationRole::User)
                    .set_content(Some(content))
                    .build()?)
            }
            Message::Assistant { content } => {
                let content = content
                    .iter()
                    .map(Vec::try_from)
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flatten()
                    .collect();

                Ok(BedrockMessage::builder()
                    .role(ConversationRole::Assistant)
                    .set_content(Some(content))
                    .build()?)
            }
        }
    }
}

pub fn messages_to_bedrock_messages(
    messages: &[Message],
) -> anyhow::Result<Option<Vec<BedrockMessage>>> {
    let bedrock_messages: Vec<BedrockMessage> = messages
        .iter()
        .map(BedrockMessage::try_from)
        .collect::<Result<_, _>>()?;

    Ok(if bedrock_messages.is_empty() {
        None
    } else {
        Some(bedrock_messages)
    })
}
