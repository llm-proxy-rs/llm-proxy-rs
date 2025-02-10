use aws_sdk_bedrockruntime::types::{ConversationRole, Message as BedrockMessage};
use request::{Message as RequestMessage, Role};

use crate::content_blocks::request_contents_to_bedrock_content_block;

pub fn request_message_to_bedrock_message(
    request_message: &RequestMessage,
) -> Option<BedrockMessage> {
    BedrockMessage::builder()
        .set_role(match request_message.role {
            Role::Assistant => Some(ConversationRole::Assistant),
            Role::User => Some(ConversationRole::User),
            _ => None,
        })
        .set_content(Some(request_contents_to_bedrock_content_block(
            &request_message.contents,
        )))
        .build()
        .ok()
}
