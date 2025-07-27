use anyhow::Result;
use aws_sdk_bedrockruntime::types::{
    Message, SystemContentBlock, Tool as BedrockTool, ToolChoice as BedrockToolChoice,
    ToolConfiguration,
};
use request::{ChatCompletionsRequest, Role, Tool, ToolChoice};

pub struct BedrockChatCompletion {
    pub model_id: String,
    pub messages: Vec<Message>,
    pub system_content_blocks: Vec<SystemContentBlock>,
    pub tool_config: Option<ToolConfiguration>,
}

pub fn process_chat_completions_request_to_bedrock_chat_completion(
    request: &ChatCompletionsRequest,
) -> Result<BedrockChatCompletion> {
    let mut system_content_blocks = Vec::new();
    let mut messages = Vec::new();

    for request_message in &request.messages {
        match request_message.role {
            Role::Assistant | Role::Tool | Role::User => {
                messages.push(Message::try_from(request_message)?);
            }
            Role::System => {
                if let Some(contents) = &request_message.contents {
                    system_content_blocks.extend::<Vec<SystemContentBlock>>(contents.into());
                }
            }
        }
    }

    let tool_config = build_tool_configuration(&request.tools, &request.tool_choice)?;

    Ok(BedrockChatCompletion {
        model_id: request.model.clone(),
        messages,
        system_content_blocks,
        tool_config,
    })
}

fn build_tool_configuration(
    tools: &Option<Vec<Tool>>,
    tool_choice: &Option<ToolChoice>,
) -> Result<Option<ToolConfiguration>> {
    if tools.is_none() && tool_choice.is_none() {
        return Ok(None);
    }

    let mut builder = ToolConfiguration::builder();

    if let Some(tools) = tools {
        for tool in tools {
            let bedrock_tool = BedrockTool::try_from(tool)?;
            builder = builder.tools(bedrock_tool);
        }
    }

    if let Some(tool_choice) = tool_choice {
        let bedrock_tool_choice = Option::<BedrockToolChoice>::try_from(tool_choice)?;
        builder = builder.set_tool_choice(bedrock_tool_choice);
    }

    Ok(Some(builder.build()?))
}
