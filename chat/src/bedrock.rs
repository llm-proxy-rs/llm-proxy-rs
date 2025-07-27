use anyhow::Result;
use aws_sdk_bedrockruntime::types::{
    AnyToolChoice, AutoToolChoice, Message, SpecificToolChoice, SystemContentBlock,
    Tool as BedrockTool, ToolChoice as BedrockToolChoice, ToolConfiguration, ToolInputSchema,
    ToolSpecification,
};
use request::{ChatCompletionsRequest, Role, Tool, ToolChoice, value_to_document};

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

    let tool_config = request
        .tools
        .as_ref()
        .map(|tools| openai_tools_to_bedrock_tool_config(tools, &request.tool_choice))
        .transpose()?;

    Ok(BedrockChatCompletion {
        model_id: request.model.clone(),
        messages,
        system_content_blocks,
        tool_config,
    })
}

fn openai_tools_to_bedrock_tool_config(
    openai_tools: &[Tool],
    openai_tool_choice: &Option<ToolChoice>,
) -> Result<ToolConfiguration> {
    let mut builder = ToolConfiguration::builder();

    for openai_tool in openai_tools {
        let tool_spec = ToolSpecification::builder()
            .name(&openai_tool.function.name)
            .set_description(openai_tool.function.description.clone())
            .input_schema(ToolInputSchema::Json(value_to_document(
                &openai_tool.function.parameters,
            )))
            .build()?;

        builder = builder.tools(BedrockTool::ToolSpec(tool_spec));
    }

    if let Some(openai_tool_choice) = openai_tool_choice {
        let bedrock_tool_choice = match openai_tool_choice {
            ToolChoice::String(s) => match s.as_str() {
                "none" => None,
                "required" => Some(BedrockToolChoice::Any(AnyToolChoice::builder().build())),
                _ => Some(BedrockToolChoice::Auto(AutoToolChoice::builder().build())),
            },
            ToolChoice::Object { function, .. } => Some(BedrockToolChoice::Tool(
                SpecificToolChoice::builder().name(&function.name).build()?,
            )),
        };
        builder = builder.set_tool_choice(bedrock_tool_choice);
    }

    Ok(builder.build()?)
}
