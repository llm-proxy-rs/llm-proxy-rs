use anyhow::Result;
use aws_sdk_bedrockruntime::types::{
    CachePointBlock, CachePointType, ContentBlock as BedrockContentBlock, ConversationRole,
    Message as BedrockMessage,
};
use serde::{Deserialize, Serialize};

use crate::content::ContentBlock;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Message {
    pub role: Role,
    pub content: Content,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum Content {
    String(String),
    Blocks(Vec<ContentBlock>),
}

// Trait implementations for conversions to Bedrock types

impl From<&Role> for ConversationRole {
    fn from(role: &Role) -> Self {
        match role {
            Role::User => ConversationRole::User,
            Role::Assistant => ConversationRole::Assistant,
        }
    }
}

impl TryFrom<&Message> for BedrockMessage {
    type Error = anyhow::Error;

    fn try_from(message: &Message) -> Result<Self, Self::Error> {
        let role = ConversationRole::from(&message.role);

        let content_blocks = match &message.content {
            Content::String(s) => vec![BedrockContentBlock::Text(s.clone())],
            Content::Blocks(blocks) => {
                let mut result = Vec::new();
                for block in blocks {
                    // Skip thinking blocks - they should not be sent to Bedrock
                    if matches!(block, ContentBlock::Thinking { .. }) {
                        continue;
                    }

                    // Convert the content block
                    if let Ok(bedrock_block) = BedrockContentBlock::try_from(block) {
                        result.push(bedrock_block);

                        // Insert cache point if this block has cache_control
                        let has_cache_control = match block {
                            ContentBlock::Text { cache_control, .. } => cache_control.is_some(),
                            ContentBlock::Image { cache_control, .. } => cache_control.is_some(),
                            ContentBlock::Document { cache_control, .. } => cache_control.is_some(),
                            _ => false,
                        };

                        if has_cache_control {
                            let cache_point = CachePointBlock::builder()
                                .r#type(CachePointType::Default)
                                .build()
                                .expect("Failed to build cache point");
                            result.push(BedrockContentBlock::CachePoint(cache_point));
                        }
                    }
                }
                result
            }
        };

        Ok(BedrockMessage::builder()
            .role(role)
            .set_content(Some(content_blocks))
            .build()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_message_content_string() {
        let json = json!({
            "role": "user",
            "content": "Hello, world!"
        });

        let message: Result<Message, _> = serde_json::from_value(json);
        assert!(
            message.is_ok(),
            "Should deserialize string content: {:?}",
            message.err()
        );
    }

    #[test]
    fn test_message_content_empty_string() {
        let json = json!({
            "role": "assistant",
            "content": ""
        });

        let message: Result<Message, _> = serde_json::from_value(json);
        assert!(
            message.is_ok(),
            "Should deserialize empty string: {:?}",
            message.err()
        );
    }

    #[test]
    fn test_message_content_blocks() {
        let json = json!({
            "role": "user",
            "content": [
                {"type": "text", "text": "Hello, world!"}
            ]
        });

        let message: Result<Message, _> = serde_json::from_value(json);
        assert!(
            message.is_ok(),
            "Should deserialize block content: {:?}",
            message.err()
        );
    }

    #[test]
    fn test_message_content_empty_array() {
        let json = json!({
            "role": "assistant",
            "content": []
        });

        let message: Result<Message, _> = serde_json::from_value(json);
        assert!(
            message.is_ok(),
            "Should deserialize empty array: {:?}",
            message.err()
        );
    }

    #[test]
    fn test_serialize_assistant_message_with_text_block() {
        use crate::content::ContentBlock;

        let message = Message {
            role: MessageRole::Assistant,
            content: MessageContent::Blocks(vec![ContentBlock::Text {
                text: "{".to_string(),
                cache_control: None,
            }]),
        };

        let json = serde_json::to_value(&message).unwrap();
        println!(
            "Serialized JSON: {}",
            serde_json::to_string_pretty(&json).unwrap()
        );

        // Should serialize with content as array of blocks
        assert!(json["content"].is_array(), "Content should be an array");
        assert_eq!(json["content"][0]["type"], "text");
        assert_eq!(json["content"][0]["text"], "{");
    }

    #[test]
    fn test_deserialize_assistant_message_with_text_block() {
        // Simulate the JSON a client would send
        let json_str = r#"{
            "role": "assistant",
            "content": [
                {
                    "type": "text",
                    "text": "{"
                }
            ]
        }"#;

        let result: Result<Message, _> = serde_json::from_str(json_str);
        match &result {
            Ok(msg) => {
                println!("Successfully deserialized: {:?}", msg);
                match &msg.content {
                    MessageContent::Blocks(blocks) => {
                        assert_eq!(blocks.len(), 1);
                        println!("Content blocks: {:?}", blocks);
                    }
                    _ => panic!("Expected Blocks variant"),
                }
            }
            Err(e) => {
                println!("Deserialization error: {}", e);
            }
        }
        assert!(
            result.is_ok(),
            "Should deserialize JSON: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_deserialize_message_with_unknown_content_block_type() {
        // Test what happens with an unknown block type
        let json_str = r#"{
            "role": "assistant",
            "content": [
                {
                    "type": "unknown",
                    "data": "something"
                }
            ]
        }"#;

        let result: Result<Message, _> = serde_json::from_str(json_str);
        println!("Result for unknown type: {:?}", result);
        // This should fail because "unknown" is not a valid ContentBlock variant
    }

    #[test]
    fn test_round_trip_serialization() {
        use crate::content::ContentBlock;

        // Create a message, serialize it, then deserialize it
        let original = Message {
            role: MessageRole::Assistant,
            content: MessageContent::Blocks(vec![ContentBlock::Text {
                text: "{".to_string(),
                cache_control: None,
            }]),
        };

        let json_str = serde_json::to_string(&original).unwrap();
        println!("Serialized: {}", json_str);

        let deserialized: Message = serde_json::from_str(&json_str).unwrap();
        println!("Deserialized: {:?}", deserialized);

        // Verify they match
        match (&original.content, &deserialized.content) {
            (MessageContent::Blocks(orig), MessageContent::Blocks(deser)) => {
                assert_eq!(orig.len(), deser.len());
            }
            _ => panic!("Content type mismatch"),
        }
    }

    #[test]
    fn test_thinking_blocks_filtered_out_for_bedrock() {
        use crate::content::ContentBlock;

        // Create an assistant message with thinking block and text block
        let message = Message {
            role: MessageRole::Assistant,
            content: MessageContent::Blocks(vec![
                ContentBlock::Thinking {
                    thinking: "This is internal reasoning".to_string(),
                    signature: Some("sig_123".to_string()),
                },
                ContentBlock::Text {
                    text: "This is the actual response".to_string(),
                    cache_control: None,
                },
            ]),
        };

        // Convert to Bedrock message
        let bedrock_message = BedrockMessage::try_from(&message).unwrap();

        // Verify thinking block was filtered out
        assert_eq!(bedrock_message.content().len(), 1);

        // Verify only the text block remains
        match &bedrock_message.content()[0] {
            BedrockContentBlock::Text(text) => {
                assert_eq!(text, "This is the actual response");
            }
            _ => panic!("Expected Text block, got something else"),
        }
    }
}
