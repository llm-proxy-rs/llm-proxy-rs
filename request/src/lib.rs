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
        
        // Create image content using OpenAI API format
        let image_url = ImageUrl {
            url: format!("data:image/png;base64,{}", simple_png_base64),
        };
        
        // Create content with image
        let content = Content::ImageUrl { image_url };
        let contents = Contents::Array(vec![content]);
        
        // Test conversion to AWS ContentBlock
        let content_blocks: Vec<aws_sdk_bedrockruntime::types::ContentBlock> = (&contents).into();
        assert_eq!(content_blocks.len(), 1);
        
        // Test JSON serialization/deserialization
        let json = serde_json::to_string(&contents).unwrap();
        println!("JSON: {}", json);
        
        let deserialized: Contents = serde_json::from_str(&json).unwrap();
        match deserialized {
            Contents::Array(arr) => {
                assert_eq!(arr.len(), 1);
                match &arr[0] {
                    Content::ImageUrl { image_url } => {
                        assert!(image_url.url.starts_with("data:image/png;base64,"));
                    }
                    _ => panic!("Expected ImageUrl content"),
                }
            }
            _ => panic!("Expected array of contents"),
        }
    }
    
    #[test]
    fn test_mixed_content() {
        let json_input = r#"{
            "model": "gpt-4o",
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "text",
                            "text": "What's in this image?"
                        },
                        {
                            "type": "image_url",
                            "image_url": {
                                "url": "data:image/jpeg;base64,/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAYEBQYFBAYGBQYHBwYIChAKCgkJChQODwwQFxQYGBcUFhYaHSUfGhsjHBYWICwgIyYnKSopGR8tMC0oMCUoKSj/2wBDAQcHBwoIChMKChMoGhYaKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCj/wAARCAABAAEDASIAAhEBAxEB/8QAFQABAQAAAAAAAAAAAAAAAAAAAAv/xAAhEAACAQMDBQAAAAAAAAAAAAABAgMABAUGIWGRkqGx0f/EABUBAQEAAAAAAAAAAAAAAAAAAAMF/8QAGhEAAgIDAAAAAAAAAAAAAAAAAAECEgMRkf/aAAwDAQACEQMRAD8AltJagyeH0AthI5xdrLcNM91BF5pX2HaH9bcfaSXWGaRmknyJckliyjqTzSlT54b6bk+h0R+Sh7p4qGylPmIhVCOvNBgQhOI5B/8A/9k="
                            }
                        }
                    ]
                }
            ]
        }"#;
        
        let request: ChatCompletionsRequest = serde_json::from_str(json_input).unwrap();
        assert_eq!(request.messages.len(), 1);
        
        match &request.messages[0].contents {
            Some(Contents::Array(contents)) => {
                assert_eq!(contents.len(), 2);
                match &contents[0] {
                    Content::Text { text } => {
                        assert_eq!(text, "What's in this image?");
                    }
                    _ => panic!("Expected text content"),
                }
                match &contents[1] {
                    Content::ImageUrl { image_url } => {
                        assert!(image_url.url.starts_with("data:image/jpeg;base64,"));
                    }
                    _ => panic!("Expected image_url content"),
                }
            }
            _ => panic!("Expected array of contents"),
        }
    }
}
