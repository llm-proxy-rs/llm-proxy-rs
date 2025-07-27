use aws_sdk_bedrockruntime::types::{
    ContentBlock, ConversationRole, Message as BedrockMessage, ToolResultBlock, ToolUseBlock,
};
use serde::{Deserialize, Serialize};

use crate::content::Contents;

#[derive(Debug, Deserialize, Serialize)]
pub struct Message {
    #[serde(rename = "content")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contents: Option<Contents>,
    pub role: Role,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<crate::ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Assistant,
    System,
    User,
    Tool,
}

impl TryFrom<&Message> for Option<Vec<ContentBlock>> {
    type Error = anyhow::Error;

    fn try_from(message: &Message) -> Result<Self, Self::Error> {
        match message.role {
            Role::Tool => Ok(Some(vec![ContentBlock::ToolResult(
                ToolResultBlock::try_from(message)?,
            )])),
            Role::Assistant => {
                let content_blocks = message
                    .contents
                    .iter()
                    .flat_map(Vec::<ContentBlock>::from)
                    .chain(
                        message
                            .tool_calls
                            .iter()
                            .flatten()
                            .map(ToolUseBlock::try_from)
                            .collect::<Result<Vec<_>, _>>()?
                            .into_iter()
                            .map(ContentBlock::ToolUse),
                    )
                    .collect::<Vec<_>>();

                if content_blocks.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(content_blocks))
                }
            }
            Role::User => Ok(message.contents.as_ref().map(|contents| contents.into())),
            Role::System => unreachable!(),
        }
    }
}

impl TryFrom<&Message> for BedrockMessage {
    type Error = anyhow::Error;

    fn try_from(message: &Message) -> Result<Self, Self::Error> {
        match message.role {
            Role::Assistant => Ok(BedrockMessage::builder()
                .role(ConversationRole::Assistant)
                .set_content(Option::<Vec<ContentBlock>>::try_from(message)?)
                .build()?),
            Role::Tool | Role::User => Ok(BedrockMessage::builder()
                .role(ConversationRole::User)
                .set_content(Option::<Vec<ContentBlock>>::try_from(message)?)
                .build()?),
            Role::System => unreachable!(),
        }
    }
}
