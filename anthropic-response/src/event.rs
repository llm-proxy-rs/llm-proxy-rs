use serde::{Deserialize, Serialize};

use crate::content_block_delta::ContentBlockDelta;
use crate::message::Message;

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Event {
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta {
        delta: ContentBlockDelta,
        index: i32,
    },
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        content_block: ContentBlock,
        index: i32,
    },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: i32 },
    #[serde(rename = "message_delta")]
    MessageDelta {
        delta: MessageDeltaContent,
        usage: UsageDelta,
    },
    #[serde(rename = "message_start")]
    MessageStart { message: Message },
    #[serde(rename = "message_stop")]
    MessageStop,
}

impl Event {
    pub fn content_block_delta_builder() -> ContentBlockDeltaEventBuilder {
        ContentBlockDeltaEventBuilder::default()
    }

    pub fn content_block_start_builder() -> ContentBlockStartEventBuilder {
        ContentBlockStartEventBuilder::default()
    }

    pub fn content_block_stop_builder() -> ContentBlockStopEventBuilder {
        ContentBlockStopEventBuilder::default()
    }

    pub fn message_delta_builder() -> MessageDeltaEventBuilder {
        MessageDeltaEventBuilder::default()
    }

    pub fn message_start_builder() -> MessageStartEventBuilder {
        MessageStartEventBuilder::default()
    }

    pub fn message_stop() -> Self {
        Event::MessageStop
    }
}

#[derive(Default)]
pub struct ContentBlockDeltaEventBuilder {
    delta: ContentBlockDelta,
    index: i32,
}

impl ContentBlockDeltaEventBuilder {
    pub fn delta(mut self, delta: ContentBlockDelta) -> Self {
        self.delta = delta;
        self
    }

    pub fn index(mut self, index: i32) -> Self {
        self.index = index;
        self
    }

    pub fn build(self) -> Event {
        Event::ContentBlockDelta {
            delta: self.delta,
            index: self.index,
        }
    }
}

#[derive(Default)]
pub struct ContentBlockStartEventBuilder {
    content_block: ContentBlock,
    index: i32,
}

impl ContentBlockStartEventBuilder {
    pub fn content_block(mut self, content_block: ContentBlock) -> Self {
        self.content_block = content_block;
        self
    }

    pub fn index(mut self, index: i32) -> Self {
        self.index = index;
        self
    }

    pub fn build(self) -> Event {
        Event::ContentBlockStart {
            content_block: self.content_block,
            index: self.index,
        }
    }
}

#[derive(Default)]
pub struct ContentBlockStopEventBuilder {
    index: i32,
}

impl ContentBlockStopEventBuilder {
    pub fn index(mut self, index: i32) -> Self {
        self.index = index;
        self
    }

    pub fn build(self) -> Event {
        Event::ContentBlockStop { index: self.index }
    }
}

#[derive(Default)]
pub struct MessageDeltaEventBuilder {
    delta: MessageDeltaContent,
    usage: UsageDelta,
}

impl MessageDeltaEventBuilder {
    pub fn delta(mut self, delta: MessageDeltaContent) -> Self {
        self.delta = delta;
        self
    }

    pub fn usage(mut self, usage: UsageDelta) -> Self {
        self.usage = usage;
        self
    }

    pub fn build(self) -> Event {
        Event::MessageDelta {
            delta: self.delta,
            usage: self.usage,
        }
    }
}

#[derive(Default)]
pub struct MessageStartEventBuilder {
    message: Message,
}

impl MessageStartEventBuilder {
    pub fn message(mut self, message: Message) -> Self {
        self.message = message;
        self
    }

    pub fn build(self) -> Event {
        Event::MessageStart {
            message: self.message,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking { signature: String, thinking: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        input: serde_json::Value,
        name: String,
    },
}

impl Default for ContentBlock {
    fn default() -> Self {
        ContentBlock::Text {
            text: String::new(),
        }
    }
}

impl ContentBlock {
    pub fn text_builder() -> TextBlockBuilder {
        TextBlockBuilder::default()
    }

    pub fn thinking_builder() -> ThinkingBlockBuilder {
        ThinkingBlockBuilder::default()
    }

    pub fn tool_use_builder() -> ToolUseBlockBuilder {
        ToolUseBlockBuilder::default()
    }
}

#[derive(Default)]
pub struct TextBlockBuilder {
    text: String,
}

impl TextBlockBuilder {
    pub fn text(mut self, text: String) -> Self {
        self.text = text;
        self
    }

    pub fn build(self) -> ContentBlock {
        ContentBlock::Text { text: self.text }
    }
}

#[derive(Default)]
pub struct ThinkingBlockBuilder {
    signature: String,
    thinking: String,
}

impl ThinkingBlockBuilder {
    pub fn signature(mut self, signature: String) -> Self {
        self.signature = signature;
        self
    }

    pub fn thinking(mut self, thinking: String) -> Self {
        self.thinking = thinking;
        self
    }

    pub fn build(self) -> ContentBlock {
        ContentBlock::Thinking {
            signature: self.signature,
            thinking: self.thinking,
        }
    }
}

pub struct ToolUseBlockBuilder {
    id: String,
    input: serde_json::Value,
    name: String,
}

impl Default for ToolUseBlockBuilder {
    fn default() -> Self {
        Self {
            id: String::new(),
            input: serde_json::json!({}),
            name: String::new(),
        }
    }
}

impl ToolUseBlockBuilder {
    pub fn id(mut self, id: String) -> Self {
        self.id = id;
        self
    }

    pub fn input(mut self, input: serde_json::Value) -> Self {
        self.input = input;
        self
    }

    pub fn name(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    pub fn build(self) -> ContentBlock {
        ContentBlock::ToolUse {
            id: self.id,
            input: self.input,
            name: self.name,
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct MessageDeltaContent {
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct UsageDelta {
    pub input_tokens: i32,
    pub output_tokens: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<i32>,
}

impl UsageDelta {
    pub fn builder() -> UsageDeltaBuilder {
        UsageDeltaBuilder::default()
    }
}

#[derive(Default)]
pub struct UsageDeltaBuilder {
    input_tokens: i32,
    output_tokens: i32,
    cache_creation_input_tokens: Option<i32>,
    cache_read_input_tokens: Option<i32>,
}

impl UsageDeltaBuilder {
    pub fn input_tokens(mut self, input_tokens: i32) -> Self {
        self.input_tokens = input_tokens;
        self
    }

    pub fn output_tokens(mut self, output_tokens: i32) -> Self {
        self.output_tokens = output_tokens;
        self
    }

    pub fn cache_creation_input_tokens(mut self, cache_creation_input_tokens: Option<i32>) -> Self {
        self.cache_creation_input_tokens = cache_creation_input_tokens;
        self
    }

    pub fn cache_read_input_tokens(mut self, cache_read_input_tokens: Option<i32>) -> Self {
        self.cache_read_input_tokens = cache_read_input_tokens;
        self
    }

    pub fn build(self) -> UsageDelta {
        UsageDelta {
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
            cache_creation_input_tokens: self.cache_creation_input_tokens,
            cache_read_input_tokens: self.cache_read_input_tokens,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_use_builder_defaults_input_to_empty_object() {
        let block = ContentBlock::tool_use_builder()
            .id("id".to_string())
            .name("name".to_string())
            .build();

        let ContentBlock::ToolUse { input, .. } = block else {
            panic!("expected ToolUse");
        };

        assert_eq!(input, serde_json::json!({}));
    }
}
