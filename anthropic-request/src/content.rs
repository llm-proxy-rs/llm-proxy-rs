use anyhow::Result;
use aws_sdk_bedrockruntime::types::{
    CachePointBlock, CachePointType, ContentBlock as BedrockContentBlock, DocumentBlock,
    DocumentFormat, DocumentSource as BedrockDocumentSource, ImageBlock, ReasoningContentBlock,
    ReasoningTextBlock, SystemContentBlock, ToolResultBlock as BedrockToolResultBlock,
    ToolUseBlock,
};
use aws_smithy_types::Blob;
use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};

use crate::tool_result_content::{ImageSource, ToolResultContent};
use crate::{value_to_document, System};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CacheControl {
    #[serde(rename = "type")]
    pub cache_type: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    #[serde(rename = "image")]
    Image {
        source: ImageSource,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    #[serde(rename = "document")]
    Document {
        source: DocumentSource,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: ToolResultContent,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DocumentSource {
    pub data: String,
    pub media_type: String,
    #[serde(rename = "type")]
    pub source_type: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum SystemBlock {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
}

// Trait implementations for conversions to Bedrock types

impl From<&System> for Vec<SystemContentBlock> {
    fn from(system: &System) -> Self {
        match system {
            System::String(s) => vec![SystemContentBlock::Text(s.clone())],
            System::Blocks(blocks) => {
                let mut result = Vec::new();
                for block in blocks {
                    match block {
                        SystemBlock::Text {
                            text,
                            cache_control,
                        } => {
                            result.push(SystemContentBlock::Text(text.clone()));

                            // Insert cache point if this block has cache_control
                            if let Some(cache) = cache_control {
                                tracing::info!(
                                    "System block: cache_control type={}, text_length={}",
                                    cache.cache_type,
                                    text.len()
                                );
                                let cache_point = CachePointBlock::builder()
                                    .r#type(CachePointType::Default)
                                    .build()
                                    .expect("Failed to build cache point");
                                result.push(SystemContentBlock::CachePoint(cache_point));
                            }
                        }
                    }
                }
                result
            }
        }
    }
}

impl TryFrom<&ContentBlock> for BedrockContentBlock {
    type Error = anyhow::Error;

    fn try_from(block: &ContentBlock) -> Result<Self, Self::Error> {
        match block {
            ContentBlock::Text {
                text,
                cache_control,
            } => {
                if let Some(cache) = cache_control {
                    tracing::info!(
                        "Content block type: Text, cache_control: type={}, text_length={}",
                        cache.cache_type,
                        text.len()
                    );
                } else {
                    tracing::info!(
                        "Content block type: Text (no cache), text_length={}",
                        text.len()
                    );
                }
                Ok(BedrockContentBlock::Text(text.clone()))
            }
            ContentBlock::Image {
                source,
                cache_control,
            } => {
                if let Some(cache) = cache_control {
                    tracing::info!(
                        "Content block type: Image, cache_control: type={}, media_type={}",
                        cache.cache_type,
                        source.media_type
                    );
                } else {
                    tracing::info!(
                        "Content block type: Image (no cache), media_type={}",
                        source.media_type
                    );
                }
                if let Some(image_block) = Option::<ImageBlock>::from(source) {
                    Ok(BedrockContentBlock::Image(image_block))
                } else {
                    Err(anyhow::anyhow!("Failed to convert image source"))
                }
            }
            ContentBlock::Document {
                source,
                title,
                cache_control,
            } => {
                if let Some(cache) = cache_control {
                    tracing::info!(
                        "Content block type: Document, cache_control: type={}, media_type={}, title={:?}",
                        cache.cache_type,
                        source.media_type,
                        title
                    );
                } else {
                    tracing::info!(
                        "Content block type: Document (no cache), media_type={}, title={:?}",
                        source.media_type,
                        title
                    );
                }
                // Convert DocumentSource to Bedrock DocumentBlock
                let bytes = general_purpose::STANDARD.decode(&source.data)?;
                let blob = Blob::new(bytes);

                // Map media_type to DocumentFormat
                let format = match source.media_type.as_str() {
                    "application/pdf" => DocumentFormat::Pdf,
                    "text/csv" => DocumentFormat::Csv,
                    "application/msword" => DocumentFormat::Doc,
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
                        DocumentFormat::Docx
                    }
                    "application/vnd.ms-excel" => DocumentFormat::Xls,
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
                        DocumentFormat::Xlsx
                    }
                    "text/html" => DocumentFormat::Html,
                    "text/plain" => DocumentFormat::Txt,
                    "text/markdown" => DocumentFormat::Md,
                    _ => {
                        return Err(anyhow::anyhow!(
                            "Unsupported document format: {}",
                            source.media_type
                        ))
                    }
                };

                let doc_source = BedrockDocumentSource::Bytes(blob);
                let mut doc_builder = DocumentBlock::builder().format(format).source(doc_source);

                if let Some(name) = title {
                    doc_builder = doc_builder.name(name);
                }

                Ok(BedrockContentBlock::Document(doc_builder.build()?))
            }
            ContentBlock::ToolUse { id, name, input } => {
                tracing::info!("Content block type: ToolUse, id={}, name={}", id, name);
                let tool_use = ToolUseBlock::builder()
                    .tool_use_id(id)
                    .name(name)
                    .input(value_to_document(input))
                    .build()?;
                Ok(BedrockContentBlock::ToolUse(tool_use))
            }
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                tracing::info!(
                    "Content block type: ToolResult, tool_use_id={}, is_error={:?}",
                    tool_use_id,
                    is_error
                );
                let tool_result_blocks =
                    Vec::<aws_sdk_bedrockruntime::types::ToolResultContentBlock>::from(content);

                let mut builder = BedrockToolResultBlock::builder().tool_use_id(tool_use_id);
                for block in tool_result_blocks {
                    builder = builder.content(block);
                }

                if let Some(true) = is_error {
                    builder =
                        builder.status(aws_sdk_bedrockruntime::types::ToolResultStatus::Error);
                }

                Ok(BedrockContentBlock::ToolResult(builder.build()?))
            }
            ContentBlock::Thinking {
                thinking,
                signature,
            } => {
                tracing::info!(
                    "Content block type: Thinking, text_length={}, has_signature={}",
                    thinking.len(),
                    signature.is_some()
                );

                // Convert thinking blocks to Bedrock's ReasoningContentBlock
                let mut reasoning_text_builder =
                    ReasoningTextBlock::builder().text(thinking.clone());

                // Include signature if present (for multi-turn conversations)
                if let Some(sig) = signature {
                    reasoning_text_builder = reasoning_text_builder.signature(sig.clone());
                }

                let reasoning_text = reasoning_text_builder.build()?;
                let reasoning_block = ReasoningContentBlock::ReasoningText(reasoning_text);

                Ok(BedrockContentBlock::ReasoningContent(reasoning_block))
            }
        }
    }
}
