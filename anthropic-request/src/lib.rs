use serde::{Deserialize, Serialize};

pub mod additional_model_request_fields;
pub mod anthropic_beta;
pub mod cache_control;
pub mod content;
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
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<Systems>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<Thinking>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_config: Option<OutputConfig>,
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
    use aws_sdk_bedrockruntime::types::{
        ContentBlock, Message as BedrockMessage, SystemContentBlock, Tool as BedrockTool,
    };
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
        let bedrock_tools = tool::tools_to_bedrock_tools(tools).unwrap().unwrap();
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
        let m0 = BedrockMessage::try_from(&messages[0]).unwrap();
        assert_eq!(m0.content().len(), 2);
        assert!(matches!(m0.content()[0], ContentBlock::Image(_)));
        assert!(matches!(m0.content()[1], ContentBlock::Text(_)));

        // message 1: assistant with tool_use
        let m1 = BedrockMessage::try_from(&messages[1]).unwrap();
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
        let m2 = BedrockMessage::try_from(&messages[2]).unwrap();
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
}
