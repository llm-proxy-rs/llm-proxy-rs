use aws_sdk_bedrockruntime::types::{ConversationRole, Message as BedrockMessage};
use request::{Message as RequestMessage, Role};

use crate::content_blocks::ToBedrockContentBlocks;

pub trait ToBedrockMessage {
    fn to_bedrock_message(&self) -> Option<BedrockMessage>;
}

impl ToBedrockMessage for RequestMessage {
    fn to_bedrock_message(&self) -> Option<BedrockMessage> {
        BedrockMessage::builder()
            .set_role(match self.role {
                Role::Assistant => Some(ConversationRole::Assistant),
                Role::User => Some(ConversationRole::User),
                _ => None,
            })
            .set_content(Some(self.contents.to_bedrock_content_blocks()))
            .build()
            .ok()
    }
}
