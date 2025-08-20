use aws_sdk_bedrockruntime::types::{
    ContentBlock, ConversationRole, Message as BedrockMessage, ToolResultBlock, ToolUseBlock,
};
use serde::{Deserialize, Serialize};

use crate::content::{Contents, SystemContents};

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum Message {
    System {
        #[serde(rename = "content")]
        #[serde(skip_serializing_if = "Option::is_none")]
        contents: Option<SystemContents>,
    },
    User {
        #[serde(rename = "content")]
        #[serde(skip_serializing_if = "Option::is_none")]
        contents: Option<Contents>,
    },
    Assistant {
        #[serde(rename = "content")]
        #[serde(skip_serializing_if = "Option::is_none")]
        contents: Option<Contents>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_calls: Option<Vec<crate::ToolCall>>,
    },
    Tool {
        #[serde(rename = "content")]
        #[serde(skip_serializing_if = "Option::is_none")]
        contents: Option<Contents>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_call_id: Option<String>,
    },
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
        match message {
            Message::Tool { .. } => Ok(Some(vec![ContentBlock::ToolResult(
                ToolResultBlock::try_from(message)?,
            )])),
            Message::Assistant {
                contents,
                tool_calls,
            } => {
                let content_blocks = contents
                    .iter()
                    .flat_map(Vec::<ContentBlock>::from)
                    .chain(
                        tool_calls
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
            Message::User { contents } => Ok(contents.as_ref().map(|contents| contents.into())),
            Message::System { .. } => unreachable!(),
        }
    }
}

impl TryFrom<&Message> for BedrockMessage {
    type Error = anyhow::Error;

    fn try_from(message: &Message) -> Result<Self, Self::Error> {
        match message {
            Message::Assistant { .. } => Ok(BedrockMessage::builder()
                .role(ConversationRole::Assistant)
                .set_content(Option::<Vec<ContentBlock>>::try_from(message)?)
                .build()?),
            Message::Tool { .. } | Message::User { .. } => Ok(BedrockMessage::builder()
                .role(ConversationRole::User)
                .set_content(Option::<Vec<ContentBlock>>::try_from(message)?)
                .build()?),
            Message::System { .. } => unreachable!(),
        }
    }
}
