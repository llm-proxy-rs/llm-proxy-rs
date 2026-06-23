use aws_sdk_bedrockruntime::types::{ContentBlock, ConversationRole, Message as BedrockMessage};
use serde::{Deserialize, Serialize};

use crate::content::{AssistantContents, UserContents};
use crate::document_source::DocumentCounter;

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Messages {
    Array(Vec<Message>),
    String(String),
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum Message {
    Assistant { content: AssistantContents },
    User { content: UserContents },
    System { content: UserContents },
}

impl Message {
    pub fn to_bedrock_message(&self, counter: &DocumentCounter) -> anyhow::Result<BedrockMessage> {
        match self {
            Message::User { content } => {
                let all_content_blocks = content.to_content_blocks(counter)?;
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
            Message::System { content } => {
                // Claude Opus 4.8 on Bedrock rejects a `system` role in the messages list
                // ("This model doesn't support system messages. Try again without a
                // system message or use a model that supports system messages."), so
                // forward system content as a user turn instead.
                let content_blocks = content.to_content_blocks(counter)?;

                Ok(BedrockMessage::builder()
                    .role(ConversationRole::User)
                    .set_content(Some(content_blocks))
                    .build()?)
            }
        }
    }
}

impl TryFrom<&Messages> for Option<Vec<BedrockMessage>> {
    type Error = anyhow::Error;

    fn try_from(messages: &Messages) -> Result<Self, Self::Error> {
        let counter = DocumentCounter::new();
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
                .map(|m| m.to_bedrock_message(&counter))
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
        let bedrock = message.to_bedrock_message(&DocumentCounter::new()).unwrap();
        let content = bedrock.content();
        assert_eq!(content.len(), 2);
        match &content[0] {
            ContentBlock::ToolResult(result) => assert_eq!(result.tool_use_id(), "t1"),
            other => panic!("expected ToolResult, got {:?}", other),
        }
        match &content[1] {
            ContentBlock::Text(text) => assert_eq!(text, "hello"),
            other => panic!("expected Text, got {:?}", other),
        }
    }

    #[test]
    fn user_message_with_cache_control() {
        let json = serde_json::json!({
            "role": "user",
            "content": [
                {"type": "text", "text": "cached prompt", "cache_control": {"type": "ephemeral"}},
                {"type": "text", "text": "uncached part"}
            ]
        });
        let message: Message = serde_json::from_value(json).unwrap();
        let bedrock = message.to_bedrock_message(&DocumentCounter::new()).unwrap();
        assert_eq!(bedrock.role(), &ConversationRole::User);
        assert_eq!(bedrock.content().len(), 3);
        match &bedrock.content()[0] {
            ContentBlock::Text(text) => assert_eq!(text, "cached prompt"),
            other => panic!("expected Text, got {:?}", other),
        }
        assert!(matches!(bedrock.content()[1], ContentBlock::CachePoint(_)));
        match &bedrock.content()[2] {
            ContentBlock::Text(text) => assert_eq!(text, "uncached part"),
            other => panic!("expected Text, got {:?}", other),
        }
    }

    #[test]
    fn assistant_message_with_cache_control() {
        let json = serde_json::json!({
            "role": "assistant",
            "content": [
                {"type": "text", "text": "cached response", "cache_control": {"type": "ephemeral"}}
            ]
        });
        let message: Message = serde_json::from_value(json).unwrap();
        let bedrock = message.to_bedrock_message(&DocumentCounter::new()).unwrap();
        assert_eq!(bedrock.role(), &ConversationRole::Assistant);
        assert_eq!(bedrock.content().len(), 2);
        match &bedrock.content()[0] {
            ContentBlock::Text(text) => assert_eq!(text, "cached response"),
            other => panic!("expected Text, got {:?}", other),
        }
        assert!(matches!(bedrock.content()[1], ContentBlock::CachePoint(_)));
    }

    #[test]
    fn tool_result_and_text_both_cached_collapse_to_one_cache_point() {
        // Reproduces the payload that triggered the Bedrock cache-point error:
        // a tool_result and a trailing text block in one user message both carry
        // cache_control, which would emit two CachePoint blocks in one array.
        let json = serde_json::json!({
            "role": "user",
            "content": [
                {
                    "type": "tool_result",
                    "tool_use_id": "tooluse_1",
                    "content": "Edit applied successfully.",
                    "cache_control": {"type": "ephemeral"}
                },
                {
                    "type": "text",
                    "text": "Create a new anchored summary...",
                    "cache_control": {"type": "ephemeral"}
                }
            ]
        });
        let message: Message = serde_json::from_value(json).unwrap();
        let bedrock = message.to_bedrock_message(&DocumentCounter::new()).unwrap();

        let cache_points = bedrock
            .content()
            .iter()
            .filter(|b| matches!(b, ContentBlock::CachePoint(_)))
            .count();
        assert_eq!(cache_points, 1, "expected exactly one cache point");

        // The surviving cache point is the last block (caches the largest prefix).
        let content = bedrock.content();
        assert!(matches!(
            content[content.len() - 1],
            ContentBlock::CachePoint(_)
        ));
    }

    #[test]
    fn messages_string_to_bedrock() {
        let json = serde_json::json!("just a string");
        let messages: Messages = serde_json::from_value(json).unwrap();
        let bedrock = Option::<Vec<BedrockMessage>>::try_from(&messages)
            .unwrap()
            .unwrap();
        assert_eq!(bedrock.len(), 1);
        assert_eq!(bedrock[0].role(), &ConversationRole::User);
        match &bedrock[0].content()[0] {
            ContentBlock::Text(text) => assert_eq!(text, "just a string"),
            other => panic!("expected Text, got {:?}", other),
        }
    }

    #[test]
    fn system_message_converted_to_user() {
        let json = serde_json::json!({
            "role": "system",
            "content": "# MCP Server Instructions"
        });
        let message: Message = serde_json::from_value(json).unwrap();
        let bedrock = message.to_bedrock_message(&DocumentCounter::new()).unwrap();
        assert_eq!(bedrock.role(), &ConversationRole::User);
        assert_eq!(bedrock.content().len(), 1);
        match &bedrock.content()[0] {
            ContentBlock::Text(text) => assert_eq!(text, "# MCP Server Instructions"),
            other => panic!("expected Text, got {:?}", other),
        }
    }

    #[test]
    fn system_message_with_cache_control_converted_to_user() {
        let json = serde_json::json!({
            "role": "system",
            "content": [
                {"type": "text", "text": "cached instructions", "cache_control": {"type": "ephemeral"}}
            ]
        });
        let message: Message = serde_json::from_value(json).unwrap();
        let bedrock = message.to_bedrock_message(&DocumentCounter::new()).unwrap();
        assert_eq!(bedrock.role(), &ConversationRole::User);
        assert_eq!(bedrock.content().len(), 2);
        match &bedrock.content()[0] {
            ContentBlock::Text(text) => assert_eq!(text, "cached instructions"),
            other => panic!("expected Text, got {:?}", other),
        }
        assert!(matches!(bedrock.content()[1], ContentBlock::CachePoint(_)));
    }

    #[test]
    fn trailing_system_message_becomes_user_turn() {
        let json = serde_json::json!([
            {"role": "user", "content": "hi"},
            {"role": "system", "content": "instructions"}
        ]);
        let messages: Messages = serde_json::from_value(json).unwrap();
        let bedrock = Option::<Vec<BedrockMessage>>::try_from(&messages)
            .unwrap()
            .unwrap();
        assert_eq!(bedrock.len(), 2);
        assert_eq!(bedrock[0].role(), &ConversationRole::User);
        assert_eq!(bedrock[1].role(), &ConversationRole::User);
    }
}
