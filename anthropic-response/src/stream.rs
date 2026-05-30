use aws_sdk_bedrockruntime::types::{
    ContentBlockStart as BedrockContentBlockStart, ConverseStreamOutput, StopReason, TokenUsage,
};
use std::sync::Arc;

use crate::{
    content_block_delta::ContentBlockDelta,
    convert_bedrock_content_block_delta,
    event::{ContentBlock, Event, MessageDeltaContent, UsageDelta},
    message::Message,
};

pub struct EventConverter {
    message_id: String,
    model: String,
    previous_converse_stream_output_type_is_message_start_or_content_block_stop: bool,
    stop_reason: Option<String>,
    started: bool,
    terminated: bool,
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
            started: false,
            terminated: false,
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
                self.started = true;
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
                    .and_then(convert_bedrock_content_block_delta)?;

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
                self.terminated = true;

                Some(vec![
                    (
                        "message_delta",
                        Event::message_delta_builder()
                            .delta(MessageDeltaContent {
                                stop_reason: self.stop_reason.clone(),
                                stop_sequence: None,
                            })
                            .usage(
                                UsageDelta::builder()
                                    .input_tokens(
                                        event.usage.as_ref().map_or(0, |u| u.input_tokens),
                                    )
                                    .output_tokens(
                                        event.usage.as_ref().map_or(0, |u| u.output_tokens),
                                    )
                                    .cache_creation_input_tokens(
                                        event
                                            .usage
                                            .as_ref()
                                            .and_then(|u| u.cache_write_input_tokens),
                                    )
                                    .cache_read_input_tokens(
                                        event
                                            .usage
                                            .as_ref()
                                            .and_then(|u| u.cache_read_input_tokens),
                                    )
                                    .build(),
                            )
                            .build(),
                    ),
                    ("message_stop", Event::message_stop()),
                ])
            }
            _ => None,
        }
    }

    /// Synthesizes the terminating `message_delta` + `message_stop` SSE pair
    /// when the upstream Bedrock stream ended without a `Metadata` event
    /// (e.g. an intermediate gateway truncated the tail). Idempotent and a
    /// no-op once the converter has already emitted the terminator via the
    /// `Metadata` arm, or if `MessageStart` was never seen.
    ///
    /// Usage is reported as zero — the upstream `usage_callback` never fires
    /// when `Metadata` is missing, but the client at least gets a well-formed
    /// stream end with the `stop_reason` recorded from `MessageStop`.
    pub fn finalize(&mut self) -> Option<Vec<(&'static str, Event)>> {
        if !self.started || self.terminated {
            return None;
        }
        self.terminated = true;
        Some(vec![
            (
                "message_delta",
                Event::message_delta_builder()
                    .delta(MessageDeltaContent {
                        stop_reason: self.stop_reason.clone(),
                        stop_sequence: None,
                    })
                    .usage(UsageDelta::builder().build())
                    .build(),
            ),
            ("message_stop", Event::message_stop()),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_bedrockruntime::types::{
        ConversationRole, ConverseStreamMetadataEvent, MessageStartEvent, MessageStopEvent,
    };

    fn converter() -> EventConverter {
        EventConverter::new(
            "msg_test".to_string(),
            "model_test".to_string(),
            Arc::new(|_| {}),
        )
    }

    fn message_start() -> ConverseStreamOutput {
        ConverseStreamOutput::MessageStart(
            MessageStartEvent::builder()
                .role(ConversationRole::Assistant)
                .build()
                .unwrap(),
        )
    }

    fn message_stop() -> ConverseStreamOutput {
        ConverseStreamOutput::MessageStop(
            MessageStopEvent::builder()
                .stop_reason(StopReason::EndTurn)
                .build()
                .unwrap(),
        )
    }

    fn metadata() -> ConverseStreamOutput {
        ConverseStreamOutput::Metadata(ConverseStreamMetadataEvent::builder().build())
    }

    #[test]
    fn finalize_emits_terminator_when_metadata_missing() {
        let mut conv = converter();
        let _ = conv.convert(&message_start());
        let _ = conv.convert(&message_stop());

        let events = conv.finalize().expect("finalize should emit a terminator");

        let names: Vec<_> = events.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, vec!["message_delta", "message_stop"]);

        let (_, delta) = &events[0];
        let json = serde_json::to_value(delta).unwrap();
        assert_eq!(json["delta"]["stop_reason"], "end_turn");
        assert_eq!(json["usage"]["input_tokens"], 0);
        assert_eq!(json["usage"]["output_tokens"], 0);
    }

    #[test]
    fn finalize_is_noop_after_metadata_already_terminated_stream() {
        let mut conv = converter();
        let _ = conv.convert(&message_start());
        let _ = conv.convert(&message_stop());
        let _ = conv.convert(&metadata());

        assert!(conv.finalize().is_none());
    }

    #[test]
    fn finalize_is_noop_before_message_start() {
        let mut conv = converter();
        assert!(conv.finalize().is_none());
    }

    #[test]
    fn finalize_is_idempotent() {
        let mut conv = converter();
        let _ = conv.convert(&message_start());
        let _ = conv.convert(&message_stop());

        assert!(conv.finalize().is_some());
        assert!(conv.finalize().is_none());
    }
}
