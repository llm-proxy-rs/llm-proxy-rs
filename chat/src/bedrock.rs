use aws_sdk_bedrockruntime::types::{
    AnyToolChoice, AutoToolChoice, Message, SpecificToolChoice, SystemContentBlock, Tool,
    ToolChoice, ToolConfiguration, ToolInputSchema, ToolSpecification,
};
use aws_smithy_types::Document;
use request::{ChatCompletionsRequest, OpenAITool, OpenAIToolChoice, Role};
use std::collections::HashMap;

pub struct BedrockChatCompletion {
    pub model_id: String,
    pub system_content_blocks: Vec<SystemContentBlock>,
    pub messages: Vec<Message>,
    pub tool_config: Option<ToolConfiguration>,
}

pub fn process_chat_completions_request_to_bedrock_chat_completion(
    request: &ChatCompletionsRequest,
) -> BedrockChatCompletion {
    let mut system_content_blocks = Vec::new();
    let mut messages = Vec::new();
    let model_id = request.model.clone();

    for request_message in &request.messages {
        match request_message.role {
            Role::Assistant | Role::User | Role::Tool => {
                if let Ok(message) = Message::try_from(request_message) {
                    messages.push(message);
                }
            }
            Role::System => {
                let new_system_content_blocks: Vec<SystemContentBlock> =
                    (&request_message.contents).into();
                system_content_blocks.extend(new_system_content_blocks);
            }
        }
    }

    // Convert OpenAI tools to Bedrock ToolConfiguration
    let tool_config = request
        .tools
        .as_ref()
        .map(|tools| convert_openai_tools_to_bedrock_tool_config(tools, &request.tool_choice));

    BedrockChatCompletion {
        model_id,
        system_content_blocks,
        messages,
        tool_config,
    }
}

fn convert_openai_tools_to_bedrock_tool_config(
    openai_tools: &[OpenAITool],
    tool_choice: &Option<OpenAIToolChoice>,
) -> ToolConfiguration {
    let mut builder = ToolConfiguration::builder();

    // Convert tools
    for openai_tool in openai_tools {
        let tool_spec = ToolSpecification::builder()
            .name(&openai_tool.function.name)
            .set_description(openai_tool.function.description.clone())
            .input_schema(ToolInputSchema::Json(convert_json_value_to_document(
                &openai_tool.function.parameters,
            )))
            .build()
            .expect("Failed to build ToolSpecification");

        builder = builder.tools(Tool::ToolSpec(tool_spec));
    }

    // Convert tool choice
    if let Some(choice) = tool_choice {
        let bedrock_choice = match choice {
            OpenAIToolChoice::String(s) => match s.as_str() {
                "auto" => ToolChoice::Auto(AutoToolChoice::builder().build()),
                "required" | "any" => ToolChoice::Any(AnyToolChoice::builder().build()),
                _ => ToolChoice::Auto(AutoToolChoice::builder().build()), // Default to auto
            },
            OpenAIToolChoice::Object { function, .. } => ToolChoice::Tool(
                SpecificToolChoice::builder()
                    .name(&function.name)
                    .build()
                    .expect("Failed to build SpecificToolChoice"),
            ),
        };
        builder = builder.tool_choice(bedrock_choice);
    }

    builder.build().expect("Failed to build ToolConfiguration")
}

fn convert_json_value_to_document(value: &serde_json::Value) -> Document {
    match value {
        serde_json::Value::Null => Document::Null,
        serde_json::Value::Bool(b) => Document::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i >= 0 {
                    Document::Number(aws_smithy_types::Number::PosInt(i as u64))
                } else {
                    Document::Number(aws_smithy_types::Number::NegInt(i))
                }
            } else if let Some(f) = n.as_f64() {
                Document::Number(aws_smithy_types::Number::Float(f))
            } else {
                Document::Null
            }
        }
        serde_json::Value::String(s) => Document::String(s.clone()),
        serde_json::Value::Array(arr) => {
            Document::Array(arr.iter().map(convert_json_value_to_document).collect())
        }
        serde_json::Value::Object(obj) => {
            let mut map = HashMap::new();
            for (k, v) in obj {
                map.insert(k.clone(), convert_json_value_to_document(v));
            }
            Document::Object(map)
        }
    }
}
