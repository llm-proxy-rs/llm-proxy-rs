use aws_sdk_bedrockruntime::types::{
    ContentBlockDelta as BedrockContentBlockDelta, ReasoningContentBlockDelta,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ContentBlockDelta {
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
    #[serde(rename = "signature_delta")]
    SignatureDelta { signature: String },
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "thinking_delta")]
    ThinkingDelta { thinking: String },
}

pub fn bedrock_content_block_delta_to_content_block_delta(
    delta: &BedrockContentBlockDelta,
) -> Option<ContentBlockDelta> {
    match delta {
        BedrockContentBlockDelta::ReasoningContent(reasoning_content) => match reasoning_content {
            ReasoningContentBlockDelta::Signature(signature) => {
                Some(ContentBlockDelta::SignatureDelta {
                    signature: signature.clone(),
                })
            }
            ReasoningContentBlockDelta::Text(text) => Some(ContentBlockDelta::ThinkingDelta {
                thinking: text.clone(),
            }),
            _ => None,
        },
        BedrockContentBlockDelta::Text(text) => {
            Some(ContentBlockDelta::TextDelta { text: text.clone() })
        }
        BedrockContentBlockDelta::ToolUse(tool_use) => Some(ContentBlockDelta::InputJsonDelta {
            partial_json: tool_use.input.clone(),
        }),
        _ => None,
    }
}
