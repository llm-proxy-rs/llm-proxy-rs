use aws_sdk_bedrockruntime::types::{
    ContentBlockStart as BedrockContentBlockStart, ConverseStreamOutput, StopReason, TokenUsage,
};
use std::sync::Arc;

use crate::{
    bedrock_content_block_delta_to_content_block_delta,
    content_block_delta::ContentBlockDelta,
    event::{ContentBlock, Event, MessageDeltaContent, UsageDelta},
    message::Message,
};

pub struct EventConverter {
    message_id: String,
    model: String,
    previous_converse_stream_output_type_is_message_start_or_content_block_stop: bool,
    stop_reason: Option<String>,
    usage_callback: Arc<dyn Fn(&TokenUsage) + Send + Sync>,
}

impl EventConverter {
    pub fn new(
        message_id: String,
        model: String,
        usage_callback: Arc<dyn Fn(&TokenUsage) + Send + Sync>,
    ) -> Self {
        Self {
            message_id,
            model,
            previous_converse_stream_output_type_is_message_start_or_content_block_stop: false,
            stop_reason: None,
            usage_callback,
        }
    }

    pub fn convert(
        &mut self,
        converse_stream_output: &ConverseStreamOutput,
    ) -> Option<Vec<(&'static str, Event)>> {
        match converse_stream_output {
            ConverseStreamOutput::MessageStart(_) => {
                self.previous_converse_stream_output_type_is_message_start_or_content_block_stop =
                    true;
                Some(vec![(
                    "message_start",
                    Event::message_start_builder()
                        .message(
                            Message::builder()
                                .id(self.message_id.clone())
                                .model(self.model.clone())
                                .role("assistant".to_string())
                                .message_type("message".to_string())
                                .build(),
                        )
                        .build(),
                )])
            }
            ConverseStreamOutput::ContentBlockStart(event) => {
                self.previous_converse_stream_output_type_is_message_start_or_content_block_stop =
                    false;
                event
                    .start
                    .as_ref()
                    .and_then(|start| match start {
                        BedrockContentBlockStart::ToolUse(tool_use) => Some(
                            ContentBlock::tool_use_builder()
                                .id(tool_use.tool_use_id().to_string())
                                .name(tool_use.name().to_string())
                                .build(),
                        ),
                        _ => None,
                    })
                    .map(|content_block| {
                        vec![(
                            "content_block_start",
                            Event::content_block_start_builder()
                                .content_block(content_block)
                                .index(event.content_block_index)
                                .build(),
                        )]
                    })
            }
            ConverseStreamOutput::ContentBlockDelta(event) => {
                let delta = event
                    .delta
                    .as_ref()
                    .and_then(bedrock_content_block_delta_to_content_block_delta)?;

                let mut events = vec![];

                if self.previous_converse_stream_output_type_is_message_start_or_content_block_stop
                    && let Some(content_block) = match &delta {
                        ContentBlockDelta::TextDelta { .. } => {
                            Some(ContentBlock::text_builder().text(String::new()).build())
                        }
                        ContentBlockDelta::ThinkingDelta { .. }
                        | ContentBlockDelta::SignatureDelta { .. } => Some(
                            ContentBlock::thinking_builder()
                                .thinking(String::new())
                                .signature(String::new())
                                .build(),
                        ),
                        _ => None,
                    }
                {
                    events.push((
                        "content_block_start",
                        Event::content_block_start_builder()
                            .content_block(content_block)
                            .index(event.content_block_index)
                            .build(),
                    ));
                }

                self.previous_converse_stream_output_type_is_message_start_or_content_block_stop =
                    false;

                if let ContentBlockDelta::InputJsonDelta { partial_json } = &delta
                    && partial_json.is_empty()
                {
                    return None;
                }

                events.push((
                    "content_block_delta",
                    Event::content_block_delta_builder()
                        .delta(delta)
                        .index(event.content_block_index)
                        .build(),
                ));

                Some(events)
            }
            ConverseStreamOutput::ContentBlockStop(event) => {
                self.previous_converse_stream_output_type_is_message_start_or_content_block_stop =
                    true;
                Some(vec![(
                    "content_block_stop",
                    Event::content_block_stop_builder()
                        .index(event.content_block_index)
                        .build(),
                )])
            }
            ConverseStreamOutput::MessageStop(event) => {
                self.stop_reason = match event.stop_reason {
                    StopReason::EndTurn => Some("end_turn".to_string()),
                    StopReason::MaxTokens => Some("max_tokens".to_string()),
                    StopReason::StopSequence => Some("stop_sequence".to_string()),
                    StopReason::ToolUse => Some("tool_use".to_string()),
                    _ => None,
                };
                None
            }
            ConverseStreamOutput::Metadata(event) => {
                if let Some(ref usage) = event.usage {
                    (self.usage_callback)(usage);
                }

                Some(vec![
                    (
                        "message_delta",
                        Event::message_delta_builder()
                            .delta(MessageDeltaContent {
                                stop_reason: self.stop_reason.clone(),
                                stop_sequence: None,
                            })
                            .usage(UsageDelta {
                                output_tokens: event.usage.as_ref().map_or(0, |u| u.output_tokens),
                            })
                            .build(),
                    ),
                    ("message_stop", Event::message_stop()),
                ])
            }
            _ => None,
        }
    }
}
