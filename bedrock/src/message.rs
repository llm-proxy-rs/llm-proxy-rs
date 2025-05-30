use aws_sdk_bedrockruntime::types::{ConversationRole, Message as BedrockMessage};
use request::{Message as RequestMessage, Role};

pub fn request_message_to_bedrock_message(
    request_message: &RequestMessage,
) -> Option<BedrockMessage> {
    BedrockMessage::builder()
        .set_role(match request_message.role {
            Role::Assistant => Some(ConversationRole::Assistant),
            Role::User => Some(ConversationRole::User),
            _ => None,
        })
        .set_content(Some((&request_message.contents).into()))
        .build()
        .ok()
}
