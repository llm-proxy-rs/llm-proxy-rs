use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Message {
    pub content: Vec<serde_json::Value>,
    pub id: String,
    pub model: String,
    pub role: String,
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
    #[serde(rename = "type")]
    pub message_type: String,
    pub usage: Usage,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Usage {
    pub input_tokens: i32,
    pub output_tokens: i32,
}

impl Message {
    pub fn builder() -> MessageBuilder {
        MessageBuilder::default()
    }
}

#[derive(Default)]
pub struct MessageBuilder {
    content: Vec<serde_json::Value>,
    id: String,
    model: String,
    role: String,
    stop_reason: Option<String>,
    stop_sequence: Option<String>,
    message_type: String,
    usage: Usage,
}

impl MessageBuilder {
    pub fn content(mut self, content: Vec<serde_json::Value>) -> Self {
        self.content = content;
        self
    }

    pub fn id(mut self, id: String) -> Self {
        self.id = id;
        self
    }

    pub fn model(mut self, model: String) -> Self {
        self.model = model;
        self
    }

    pub fn role(mut self, role: String) -> Self {
        self.role = role;
        self
    }

    pub fn stop_reason(mut self, stop_reason: Option<String>) -> Self {
        self.stop_reason = stop_reason;
        self
    }

    pub fn stop_sequence(mut self, stop_sequence: Option<String>) -> Self {
        self.stop_sequence = stop_sequence;
        self
    }

    pub fn message_type(mut self, message_type: String) -> Self {
        self.message_type = message_type;
        self
    }

    pub fn usage(mut self, usage: Usage) -> Self {
        self.usage = usage;
        self
    }

    pub fn build(self) -> Message {
        Message {
            content: self.content,
            id: self.id,
            model: self.model,
            role: self.role,
            stop_reason: self.stop_reason,
            stop_sequence: self.stop_sequence,
            message_type: self.message_type,
            usage: self.usage,
        }
    }
}

impl Usage {
    pub fn builder() -> UsageBuilder {
        UsageBuilder::default()
    }
}

#[derive(Default)]
pub struct UsageBuilder {
    input_tokens: i32,
    output_tokens: i32,
}

impl UsageBuilder {
    pub fn input_tokens(mut self, tokens: i32) -> Self {
        self.input_tokens = tokens;
        self
    }

    pub fn output_tokens(mut self, tokens: i32) -> Self {
        self.output_tokens = tokens;
        self
    }

    pub fn build(self) -> Usage {
        Usage {
            input_tokens: self.input_tokens,
            output_tokens: self.output_tokens,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_builder_defaults_usage_to_zero() {
        let message = Message::builder()
            .id("msg_1".to_string())
            .model("claude".to_string())
            .role("assistant".to_string())
            .message_type("message".to_string())
            .build();

        assert_eq!(message.usage.input_tokens, 0);
        assert_eq!(message.usage.output_tokens, 0);
    }
}
