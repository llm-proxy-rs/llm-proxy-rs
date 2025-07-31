use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod tool;
pub use tool::*;

pub mod content;
pub use content::*;

pub mod message;
pub use message::*;

#[derive(Debug, Deserialize, Serialize)]
pub struct ChatCompletionsRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<StreamOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StreamOptions {
    pub include_usage: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_image_support() {
        // Example of a simple 1x1 PNG image as base64
        let simple_png_base64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChAI9jU77yQAAAABJRU5ErkJggg==";
        
        // Create image content
        let image_content = ImageContent {
            format: "png".to_string(),
            data: simple_png_base64.to_string(),
        };
        
        // Create content with image
        let content = Content::Image { image: image_content };
        let contents = Contents::Array(vec![content]);
        
        // Test conversion to AWS ContentBlock
        let content_blocks: Vec<aws_sdk_bedrockruntime::types::ContentBlock> = (&contents).into();
        assert_eq!(content_blocks.len(), 1);
        
        // Test with mixed content (text + image)
        let mixed_contents = Contents::Array(vec![
            Content::Text { text: "Here's an image:".to_string() },
            Content::Image { 
                image: ImageContent {
                    format: "png".to_string(),
                    data: simple_png_base64.to_string(),
                }
            },
        ]);
        
        let mixed_blocks: Vec<aws_sdk_bedrockruntime::types::ContentBlock> = (&mixed_contents).into();
        assert_eq!(mixed_blocks.len(), 2);
        
        println!("Image support test completed successfully! 🎉");
    }
}
