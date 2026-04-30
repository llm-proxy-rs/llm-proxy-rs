use serde::{Deserialize, Serialize};

pub mod additional_model_request_fields;
pub mod anthropic_beta;
pub mod cache_control;
pub mod content;
pub mod context_management;
pub mod document_source;
pub mod image_source;
pub mod message;
pub mod output_config;
pub mod system;
pub mod thinking;
pub mod tool;
pub mod tool_result_content;

pub use additional_model_request_fields::*;
pub use cache_control::*;
pub use content::*;
pub use context_management::*;
pub use document_source::*;
pub use image_source::*;
pub use message::*;
pub use output_config::*;
pub use system::*;
pub use thinking::*;
pub use tool::*;
pub use tool_result_content::*;

#[derive(Debug, Deserialize, Serialize)]
pub struct V1MessagesRequest {
    pub max_tokens: i32,
    pub messages: Messages,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<Systems>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<Thinking>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_config: Option<OutputConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_management: Option<ContextManagement>,
}

impl V1MessagesRequest {
    /// Clears the `thinking` text on every assistant `Thinking` content block in
    /// `messages`, leaving `signature` untouched. Used on retry when Bedrock rejects
    /// prior thinking blocks as modified.
    pub fn blank_assistant_thinking_text(&mut self) {
        let Messages::Array(messages) = &mut self.messages else {
            return;
        };
        for message in messages {
            let Message::Assistant { content } = message else {
                continue;
            };
            let AssistantContents::Array(blocks) = content else {
                continue;
            };
            for block in blocks {
                if let AssistantContent::Thinking { thinking, .. } = block {
                    thinking.clear();
                }
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct V1MessagesCountTokensRequest {
    pub messages: Messages,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<Systems>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<Thinking>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
}

#[cfg(test)]
mod tests {
    use aws_sdk_bedrockruntime::types::{ContentBlock, SystemContentBlock, Tool as BedrockTool};
    use base64::{Engine as _, engine::general_purpose};

    use super::*;

    #[test]
    fn v1_messages_request_with_tool_use_image_and_cache() {
        let png_data = general_purpose::STANDARD.encode([0x89, 0x50, 0x4E, 0x47]);

        let json = serde_json::json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 1024,
            "system": [
                {
                    "type": "text",
                    "text": "You are a helpful assistant with vision capabilities.",
                    "cache_control": {"type": "ephemeral"}
                }
            ],
            "tools": [
                {
                    "name": "analyze_image",
                    "description": "Analyzes an image and returns a description.",
                    "input_schema": {
                        "type": "object",
                        "properties": {
                            "query": {"type": "string"}
                        },
                        "required": ["query"]
                    },
                    "cache_control": {"type": "ephemeral"}
                }
            ],
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/png",
                                "data": png_data
                            }
                        },
                        {
                            "type": "text",
                            "text": "What is in this image?"
                        }
                    ]
                },
                {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "tool_use",
                            "id": "toolu_01",
                            "name": "analyze_image",
                            "input": {"query": "describe contents"}
                        }
                    ]
                },
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "tool_result",
                            "tool_use_id": "toolu_01",
                            "content": [
                                {"type": "text", "text": "The image contains a logo."},
                                {
                                    "type": "image",
                                    "source": {
                                        "type": "base64",
                                        "media_type": "image/png",
                                        "data": png_data
                                    }
                                }
                            ],
                            "cache_control": {"type": "ephemeral"}
                        }
                    ]
                }
            ]
        });

        let request: V1MessagesRequest = serde_json::from_value(json).unwrap();

        assert_eq!(request.model, "claude-sonnet-4-20250514");
        assert_eq!(request.max_tokens, 1024);

        // system with cache control
        let system = request.system.as_ref().unwrap();
        let system_blocks = Vec::<SystemContentBlock>::try_from(system).unwrap();
        assert_eq!(system_blocks.len(), 2);
        assert!(matches!(&system_blocks[0], SystemContentBlock::Text(t) if t.contains("vision")));
        assert!(matches!(
            system_blocks[1],
            SystemContentBlock::CachePoint(_)
        ));

        // tools with cache control
        let tools = request.tools.as_ref().unwrap();
        let bedrock_tools = tool::build_bedrock_tools(tools).unwrap().unwrap();
        assert_eq!(bedrock_tools.len(), 2);
        match &bedrock_tools[0] {
            BedrockTool::ToolSpec(spec) => assert_eq!(spec.name(), "analyze_image"),
            other => panic!("expected ToolSpec, got {:?}", other),
        }
        assert!(matches!(bedrock_tools[1], BedrockTool::CachePoint(_)));

        let messages = match &request.messages {
            Messages::Array(a) => a,
            _ => panic!("expected Array"),
        };
        assert_eq!(messages.len(), 3);

        // message 0: user with image + text
        let counter = DocumentCounter::new();
        let m0 = messages[0].to_bedrock_message(&counter).unwrap();
        assert_eq!(m0.content().len(), 2);
        assert!(matches!(m0.content()[0], ContentBlock::Image(_)));
        assert!(matches!(m0.content()[1], ContentBlock::Text(_)));

        // message 1: assistant with tool_use
        let m1 = messages[1].to_bedrock_message(&counter).unwrap();
        assert_eq!(m1.content().len(), 1);
        match &m1.content()[0] {
            ContentBlock::ToolUse(tu) => {
                assert_eq!(tu.tool_use_id(), "toolu_01");
                assert_eq!(tu.name(), "analyze_image");
            }
            other => panic!("expected ToolUse, got {:?}", other),
        }

        // message 2: user with tool_result (mixed text+image) + cache control
        // tool_result is reordered before other content; cache point follows
        let m2 = messages[2].to_bedrock_message(&counter).unwrap();
        assert_eq!(m2.content().len(), 2);
        match &m2.content()[0] {
            ContentBlock::ToolResult(tr) => {
                assert_eq!(tr.tool_use_id(), "toolu_01");
                assert_eq!(tr.content().len(), 2);
            }
            other => panic!("expected ToolResult, got {:?}", other),
        }
        assert!(matches!(m2.content()[1], ContentBlock::CachePoint(_)));
    }

    #[test]
    fn blank_assistant_thinking_text_clears_thinking_preserves_signature() {
        let json = serde_json::json!({
            "model": "claude-opus-4-7",
            "max_tokens": 1024,
            "messages": [
                {"role": "user", "content": "hello"},
                {
                    "role": "assistant",
                    "content": [
                        {"type": "thinking", "thinking": "secret reasoning", "signature": "sig-abc"},
                        {"type": "text", "text": "hi"}
                    ]
                },
                {"role": "user", "content": "next"}
            ]
        });
        let mut request: V1MessagesRequest = serde_json::from_value(json).unwrap();
        request.blank_assistant_thinking_text();

        let Messages::Array(messages) = &request.messages else {
            panic!("expected Array");
        };
        let Message::Assistant {
            content: AssistantContents::Array(blocks),
        } = &messages[1]
        else {
            panic!("expected assistant");
        };
        match &blocks[0] {
            AssistantContent::Thinking {
                thinking,
                signature,
            } => {
                assert_eq!(thinking, "");
                assert_eq!(signature, "sig-abc");
            }
            other => panic!("expected Thinking, got {:?}", other),
        }
        assert!(matches!(blocks[1], AssistantContent::Text { .. }));
    }

    #[test]
    fn blank_assistant_thinking_text_leaves_non_thinking_untouched() {
        let json = serde_json::json!({
            "model": "claude-opus-4-7",
            "max_tokens": 1024,
            "messages": [
                {
                    "role": "assistant",
                    "content": [
                        {"type": "text", "text": "hello"},
                        {"type": "redacted_thinking", "data": "opaque"}
                    ]
                }
            ]
        });
        let mut request: V1MessagesRequest = serde_json::from_value(json).unwrap();
        request.blank_assistant_thinking_text();

        let Messages::Array(messages) = &request.messages else {
            panic!("expected Array");
        };
        let Message::Assistant {
            content: AssistantContents::Array(blocks),
        } = &messages[0]
        else {
            panic!("expected assistant");
        };
        match &blocks[0] {
            AssistantContent::Text { text, .. } => assert_eq!(text, "hello"),
            other => panic!("expected Text, got {:?}", other),
        }
        match &blocks[1] {
            AssistantContent::RedactedThinking { data } => assert_eq!(data, "opaque"),
            other => panic!("expected RedactedThinking, got {:?}", other),
        }
    }

    #[test]
    fn blank_assistant_thinking_text_noop_on_string_messages() {
        let json = serde_json::json!({
            "model": "claude-opus-4-7",
            "max_tokens": 1024,
            "messages": "hello"
        });
        let mut request: V1MessagesRequest = serde_json::from_value(json).unwrap();
        request.blank_assistant_thinking_text();
        assert!(matches!(request.messages, Messages::String(ref s) if s == "hello"));
    }
}
