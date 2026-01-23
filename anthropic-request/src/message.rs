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
                let content = Vec::try_from(content)?;

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
    use crate::content::{AssistantContent, UserContent};

    #[test]
    fn test_user_message_string_content() {
        let json = r#"{
            "role": "user",
            "content": "Hello"
        }"#;

        let message: Message = serde_json::from_str(json).unwrap();
        match message {
            Message::User { content } => match content {
                UserContents::String(s) => assert_eq!(s, "Hello"),
                _ => panic!("Expected String variant"),
            },
            _ => panic!("Expected User message"),
        }
    }

    #[test]
    fn test_user_message_array_content() {
        let json = r#"{
            "role": "user",
            "content": [{"type": "text", "text": "Hello"}]
        }"#;

        let message: Message = serde_json::from_str(json).unwrap();
        match message {
            Message::User { content } => match content {
                UserContents::Array(arr) => {
                    assert_eq!(arr.len(), 1);
                    match &arr[0] {
                        UserContent::Text {
                            cache_control,
                            text,
                        } => {
                            assert_eq!(text, "Hello");
                            assert!(cache_control.is_none());
                        }
                        _ => panic!("Expected Text variant"),
                    }
                }
                _ => panic!("Expected Array variant"),
            },
            _ => panic!("Expected User message"),
        }
    }

    #[test]
    fn test_assistant_message_string_content() {
        let json = r#"{
            "role": "assistant",
            "content": "Hi there!"
        }"#;

        let message: Message = serde_json::from_str(json).unwrap();
        match message {
            Message::Assistant { content } => match content {
                AssistantContents::String(s) => assert_eq!(s, "Hi there!"),
                _ => panic!("Expected String variant"),
            },
            _ => panic!("Expected Assistant message"),
        }
    }

    #[test]
    fn test_assistant_message_array_content() {
        let json = r#"{
            "role": "assistant",
            "content": [{"type": "text", "text": "Hi there!"}]
        }"#;

        let message: Message = serde_json::from_str(json).unwrap();
        match message {
            Message::Assistant { content } => match content {
                AssistantContents::Array(arr) => {
                    assert_eq!(arr.len(), 1);
                    match &arr[0] {
                        AssistantContent::Text {
                            cache_control,
                            text,
                        } => {
                            assert_eq!(text, "Hi there!");
                            assert!(cache_control.is_none());
                        }
                        _ => panic!("Expected Text variant"),
                    }
                }
                _ => panic!("Expected Array variant"),
            },
            _ => panic!("Expected Assistant message"),
        }
    }

    #[test]
    fn test_messages_array() {
        let json = r#"[
            {
                "role": "user",
                "content": [{"type": "text", "text": "Hello"}]
            },
            {
                "role": "assistant",
                "content": [{"type": "text", "text": "Hi there!"}]
            }
        ]"#;

        let messages: Vec<Message> = serde_json::from_str(json).unwrap();
        assert_eq!(messages.len(), 2);

        match &messages[0] {
            Message::User { content } => match content {
                UserContents::Array(arr) => {
                    assert_eq!(arr.len(), 1);
                }
                _ => panic!("Expected Array variant"),
            },
            _ => panic!("Expected User message"),
        }

        match &messages[1] {
            Message::Assistant { content } => match content {
                AssistantContents::Array(arr) => {
                    assert_eq!(arr.len(), 1);
                }
                _ => panic!("Expected Array variant"),
            },
            _ => panic!("Expected Assistant message"),
        }
    }

    #[test]
    fn test_messages_string() {
        let json = r#""Hello world""#;

        let messages: Messages = serde_json::from_str(json).unwrap();
        match messages {
            Messages::String(s) => assert_eq!(s, "Hello world"),
            _ => panic!("Expected String variant"),
        }
    }

    #[test]
    fn test_messages_array_wrapper() {
        let json = r#"[
            {
                "role": "user",
                "content": "Hello"
            }
        ]"#;

        let messages: Messages = serde_json::from_str(json).unwrap();
        match messages {
            Messages::Array(arr) => {
                assert_eq!(arr.len(), 1);
            }
            _ => panic!("Expected Array variant"),
        }
    }
}
