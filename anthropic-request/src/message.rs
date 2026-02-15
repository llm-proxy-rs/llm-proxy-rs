use aws_sdk_bedrockruntime::types::{ContentBlock, ConversationRole, Message as BedrockMessage};
use serde::{Deserialize, Serialize};

use crate::content::{AssistantContents, UserContents};

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Messages {
    Array(Vec<Message>),
    String(String),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum Message {
    #[serde(rename = "assistant")]
    Assistant { content: AssistantContents },
    #[serde(rename = "user")]
    User { content: UserContents },
}

impl TryFrom<&Message> for BedrockMessage {
    type Error = anyhow::Error;

    fn try_from(message: &Message) -> Result<Self, Self::Error> {
        match message {
            Message::User { content } => {
                let all_content_blocks = Vec::try_from(content)?;
                let (tool_result_content_blocks, others_content_blocks): (Vec<_>, Vec<_>) =
                    all_content_blocks
                        .into_iter()
                        .partition(|b| matches!(b, ContentBlock::ToolResult(_)));
                let content: Vec<_> = tool_result_content_blocks
                    .into_iter()
                    .chain(others_content_blocks)
                    .collect();

                Ok(BedrockMessage::builder()
                    .role(ConversationRole::User)
                    .set_content(Some(content))
                    .build()?)
            }
            Message::Assistant { content } => {
                let content = Vec::try_from(content)?;

                Ok(BedrockMessage::builder()
                    .role(ConversationRole::Assistant)
                    .set_content(Some(content))
                    .build()?)
            }
        }
    }
}

impl TryFrom<&Messages> for Option<Vec<BedrockMessage>> {
    type Error = anyhow::Error;

    fn try_from(messages: &Messages) -> Result<Self, Self::Error> {
        let bedrock_messages: Vec<BedrockMessage> = match messages {
            Messages::String(s) => {
                let content = vec![ContentBlock::Text(s.clone())];
                vec![
                    BedrockMessage::builder()
                        .role(ConversationRole::User)
                        .set_content(Some(content))
                        .build()?,
                ]
            }
            Messages::Array(a) => a
                .iter()
                .map(BedrockMessage::try_from)
                .collect::<Result<_, _>>()?,
        };

        Ok(if bedrock_messages.is_empty() {
            None
        } else {
            Some(bedrock_messages)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_result_reordered_before_text() {
        let json = serde_json::json!({
            "role": "user",
            "content": [
                {"type": "text", "text": "hello"},
                {"type": "tool_result", "tool_use_id": "t1", "content": "result"}
            ]
        });
        let message: Message = serde_json::from_value(json).unwrap();
        let bedrock = BedrockMessage::try_from(&message).unwrap();
        let content = bedrock.content();
        assert!(matches!(content[0], ContentBlock::ToolResult(_)));
        assert!(matches!(content[1], ContentBlock::Text(_)));
    }
}
