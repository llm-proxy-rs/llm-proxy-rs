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
    /// The stop sequence reported back to the client in the terminating
    /// `message_delta`. Bedrock strips the matched sequence from the output text
    /// and does not report *which* sequence matched, so we recover it from the
    /// request's configured `stop_sequences` when the stream stops on one.
    stop_sequence: Option<String>,
    /// The `stop_sequences` configured on the originating request, used to
    /// reconstruct the matched sequence (see `stop_sequence`).
    request_stop_sequences: Option<Vec<String>>,
    /// Index of a `content_block_stop` whose emission is deferred. Bedrock sends
    /// `ContentBlockStop` *before* `MessageStop` (where we learn a stop sequence
    /// was hit), so we buffer the stop and only flush it once we know whether to
    /// inject the matched sequence as a trailing text delta first.
    pending_content_block_stop: Option<i32>,
    started: bool,
    terminated: bool,
    usage_callback: Arc<dyn Fn(&TokenUsage) + Send + Sync>,
}

impl EventConverter {
    pub fn new(
        message_id: String,
        model: String,
        request_stop_sequences: Option<Vec<String>>,
        usage_callback: Arc<dyn Fn(&TokenUsage) + Send + Sync>,
    ) -> Self {
        Self {
            message_id,
            model,
            previous_converse_stream_output_type_is_message_start_or_content_block_stop: false,
            stop_reason: None,
            stop_sequence: None,
            request_stop_sequences,
            pending_content_block_stop: None,
            started: false,
            terminated: false,
            usage_callback,
        }
    }

    /// Takes the deferred `content_block_stop`, if any. Bedrock closes one block
    /// before opening the next, so at most one stop is ever buffered — hence an
    /// `Option`, not a list.
    fn flush_pending_content_block_stop(&mut self) -> Option<(&'static str, Event)> {
        self.pending_content_block_stop.take().map(|index| {
            (
                "content_block_stop",
                Event::content_block_stop_builder().index(index).build(),
            )
        })
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
                let mut events = Vec::new();
                // Multi-block streams: Bedrock opens the next block (e.g. tool_use)
                // while the previous block's `content_block_stop` is still deferred.
                // Close that block before emitting this `content_block_start`.
                events.extend(self.flush_pending_content_block_stop());
                if let Some(content_block) = event.start.as_ref().and_then(|start| match start {
                    BedrockContentBlockStart::ToolUse(tool_use) => Some(
                        ContentBlock::tool_use_builder()
                            .id(tool_use.tool_use_id().to_string())
                            .name(tool_use.name().to_string())
                            .build(),
                    ),
                    _ => None,
                }) {
                    events.push((
                        "content_block_start",
                        Event::content_block_start_builder()
                            .content_block(content_block)
                            .index(event.content_block_index)
                            .build(),
                    ));
                }
                if events.is_empty() {
                    None
                } else {
                    Some(events)
                }
            }
            ConverseStreamOutput::ContentBlockDelta(event) => {
                let mut events = Vec::new();
                // Same as `ContentBlockStart`: the prior block's stop may still be
                // buffered when deltas for the next block arrive (Bedrock often skips
                // an explicit start and sends a delta instead).
                events.extend(self.flush_pending_content_block_stop());

                let Some(delta) = event
                    .delta
                    .as_ref()
                    .and_then(convert_bedrock_content_block_delta)
                else {
                    return if events.is_empty() {
                        None
                    } else {
                        Some(events)
                    };
                };

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
                    return if events.is_empty() {
                        None
                    } else {
                        Some(events)
                    };
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
                // Defer this stop until `MessageStop` tells us whether to inject a
                // matched stop sequence as a trailing text delta first. No earlier
                // stop can still be pending here: Bedrock always sends a
                // `ContentBlockStart`/`ContentBlockDelta` between blocks, and both
                // flush the buffer.
                self.pending_content_block_stop = Some(event.content_block_index);
                None
            }
            ConverseStreamOutput::MessageStop(event) => {
                self.stop_reason = match event.stop_reason {
                    StopReason::EndTurn => Some("end_turn".to_string()),
                    StopReason::MaxTokens => Some("max_tokens".to_string()),
                    StopReason::StopSequence => Some("stop_sequence".to_string()),
                    StopReason::ToolUse => Some("tool_use".to_string()),
                    _ => None,
                };
                // Bedrock reports that a stop sequence was hit but not which one,
                // and strips it from the output text. When the request configured
                // exactly one stop sequence the match is unambiguous, so recover
                // it for both the terminating `message_delta` and the injected
                // trailing text delta below.
                if event.stop_reason == StopReason::StopSequence
                    && let Some([only]) = self.request_stop_sequences.as_deref()
                {
                    self.stop_sequence = Some(only.clone());
                }
                // Close the open content block. If a stop sequence matched, inject
                // it as a trailing text delta *before* the deferred
                // `content_block_stop` so streaming consumers that finalize the
                // block at `content_block_stop` still see the closing text — the
                // sequence Bedrock stripped from the body. (Native Anthropic
                // reports the sequence only in `message_delta`; we additionally
                // surface it inline for parsers that don't read that frame.)
                let mut events = vec![];
                if let Some(index) = self.pending_content_block_stop.take() {
                    if let Some(seq) = &self.stop_sequence {
                        events.push((
                            "content_block_delta",
                            Event::content_block_delta_builder()
                                .delta(ContentBlockDelta::TextDelta { text: seq.clone() })
                                .index(index)
                                .build(),
                        ));
                    }
                    events.push((
                        "content_block_stop",
                        Event::content_block_stop_builder().index(index).build(),
                    ));
                }
                if events.is_empty() {
                    None
                } else {
                    Some(events)
                }
            }
            ConverseStreamOutput::Metadata(event) => {
                if let Some(ref usage) = event.usage {
                    (self.usage_callback)(usage);
                }
                self.terminated = true;

                // No `content_block_stop` can be pending: `MessageStop` always
                // precedes `Metadata` and flushes it.
                Some(vec![
                    (
                        "message_delta",
                        Event::message_delta_builder()
                            .delta(MessageDeltaContent {
                                stop_reason: self.stop_reason.clone(),
                                stop_sequence: self.stop_sequence.clone(),
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
        let mut events = Vec::new();
        // Truncated tail: `Metadata` never arrived, so `MessageStop` may not have
        // flushed the deferred stop either. Emit it before the synthetic terminator.
        events.extend(self.flush_pending_content_block_stop());
        events.push((
            "message_delta",
            Event::message_delta_builder()
                .delta(MessageDeltaContent {
                    stop_reason: self.stop_reason.clone(),
                    stop_sequence: self.stop_sequence.clone(),
                })
                .usage(UsageDelta::builder().build())
                .build(),
        ));
        events.push(("message_stop", Event::message_stop()));
        Some(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_bedrockruntime::types::{
        ContentBlockDelta as BedrockContentBlockDelta, ContentBlockDeltaEvent,
        ContentBlockStopEvent, ConversationRole, ConverseStreamMetadataEvent, MessageStartEvent,
        MessageStopEvent,
    };

    fn converter() -> EventConverter {
        EventConverter::new(
            "msg_test".to_string(),
            "model_test".to_string(),
            None,
            Arc::new(|_| {}),
        )
    }

    fn converter_with_stop_sequences(stop_sequences: Vec<String>) -> EventConverter {
        EventConverter::new(
            "msg_test".to_string(),
            "model_test".to_string(),
            Some(stop_sequences),
            Arc::new(|_| {}),
        )
    }

    fn message_stop_with(stop_reason: StopReason) -> ConverseStreamOutput {
        ConverseStreamOutput::MessageStop(
            MessageStopEvent::builder()
                .stop_reason(stop_reason)
                .build()
                .unwrap(),
        )
    }

    fn content_block_delta_text(text: &str) -> ConverseStreamOutput {
        ConverseStreamOutput::ContentBlockDelta(
            ContentBlockDeltaEvent::builder()
                .delta(BedrockContentBlockDelta::Text(text.to_string()))
                .content_block_index(0)
                .build()
                .unwrap(),
        )
    }

    fn content_block_stop() -> ConverseStreamOutput {
        ConverseStreamOutput::ContentBlockStop(
            ContentBlockStopEvent::builder()
                .content_block_index(0)
                .build()
                .unwrap(),
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

    #[test]
    fn metadata_emits_matched_stop_sequence_when_single_configured() {
        let mut conv = converter_with_stop_sequences(vec!["</block>".to_string()]);
        let _ = conv.convert(&message_start());
        let _ = conv.convert(&message_stop_with(StopReason::StopSequence));
        let events = conv.convert(&metadata()).expect("metadata emits events");

        let (_, delta) = &events[0];
        let json = serde_json::to_value(delta).unwrap();
        assert_eq!(json["delta"]["stop_reason"], "stop_sequence");
        assert_eq!(json["delta"]["stop_sequence"], "</block>");
    }

    #[test]
    fn finalize_emits_matched_stop_sequence_when_single_configured() {
        let mut conv = converter_with_stop_sequences(vec!["</block>".to_string()]);
        let _ = conv.convert(&message_start());
        let _ = conv.convert(&message_stop_with(StopReason::StopSequence));
        let events = conv.finalize().expect("finalize emits a terminator");

        let (_, delta) = &events[0];
        let json = serde_json::to_value(delta).unwrap();
        assert_eq!(json["delta"]["stop_reason"], "stop_sequence");
        assert_eq!(json["delta"]["stop_sequence"], "</block>");
    }

    #[test]
    fn stop_sequence_omitted_when_multiple_configured() {
        let mut conv =
            converter_with_stop_sequences(vec!["</block>".to_string(), "STOP".to_string()]);
        let _ = conv.convert(&message_start());
        let _ = conv.convert(&message_stop_with(StopReason::StopSequence));
        let events = conv.convert(&metadata()).expect("metadata emits events");

        let (_, delta) = &events[0];
        let json = serde_json::to_value(delta).unwrap();
        assert_eq!(json["delta"]["stop_reason"], "stop_sequence");
        // Ambiguous which sequence matched, so it is left unset.
        assert!(json["delta"]["stop_sequence"].is_null());
    }

    #[test]
    fn stop_sequence_omitted_when_stop_reason_is_not_stop_sequence() {
        let mut conv = converter_with_stop_sequences(vec!["</block>".to_string()]);
        let _ = conv.convert(&message_start());
        let _ = conv.convert(&message_stop_with(StopReason::EndTurn));
        let events = conv.convert(&metadata()).expect("metadata emits events");

        let (_, delta) = &events[0];
        let json = serde_json::to_value(delta).unwrap();
        assert_eq!(json["delta"]["stop_reason"], "end_turn");
        assert!(json["delta"]["stop_sequence"].is_null());
    }

    #[test]
    fn content_block_stop_is_deferred_until_message_stop() {
        let mut conv = converter_with_stop_sequences(vec!["</block>".to_string()]);
        let _ = conv.convert(&message_start());
        let _ = conv.convert(&content_block_delta_text("<block>no"));
        // The stop is buffered, not emitted yet.
        assert!(conv.convert(&content_block_stop()).is_none());
    }

    #[test]
    fn injects_matched_stop_sequence_as_text_delta_before_content_block_stop() {
        let mut conv = converter_with_stop_sequences(vec!["</block>".to_string()]);
        let _ = conv.convert(&message_start());
        let _ = conv.convert(&content_block_delta_text("<block>no"));
        let _ = conv.convert(&content_block_stop());

        let events = conv
            .convert(&message_stop_with(StopReason::StopSequence))
            .expect("message_stop should flush injected delta + content_block_stop");

        let names: Vec<_> = events.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vec!["content_block_delta", "content_block_stop"]);

        let (_, injected) = &events[0];
        let json = serde_json::to_value(injected).unwrap();
        assert_eq!(json["type"], "content_block_delta");
        assert_eq!(json["delta"]["type"], "text_delta");
        assert_eq!(json["delta"]["text"], "</block>");
        assert_eq!(json["index"], 0);
    }

    #[test]
    fn closes_block_without_injection_when_not_stop_sequence() {
        let mut conv = converter_with_stop_sequences(vec!["</block>".to_string()]);
        let _ = conv.convert(&message_start());
        let _ = conv.convert(&content_block_delta_text("hi"));
        let _ = conv.convert(&content_block_stop());

        let events = conv
            .convert(&message_stop_with(StopReason::EndTurn))
            .expect("buffered content_block_stop should still be emitted");

        let names: Vec<_> = events.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vec!["content_block_stop"]);
    }

    #[test]
    fn full_stream_orders_injected_close_before_stop_and_delta() {
        let mut conv = converter_with_stop_sequences(vec!["</block>".to_string()]);
        let mut names = vec![];
        for out in [
            message_start(),
            content_block_delta_text("<block>no"),
            content_block_stop(),
            message_stop_with(StopReason::StopSequence),
            metadata(),
        ] {
            if let Some(events) = conv.convert(&out) {
                names.extend(events.into_iter().map(|(n, _)| n));
            }
        }
        // The injected "</block>" delta lands after the body delta and before
        // the block closes; the message_delta (carrying stop_sequence) follows.
        assert_eq!(
            names,
            vec![
                "message_start",
                "content_block_start",
                "content_block_delta", // "<block>no"
                "content_block_delta", // injected "</block>"
                "content_block_stop",
                "message_delta",
                "message_stop",
            ]
        );
    }
}
