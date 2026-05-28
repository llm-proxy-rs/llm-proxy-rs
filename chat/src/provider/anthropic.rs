use anthropic_request::{
    AssistantContent, AssistantContents, Message, Messages, UserContent, UserContents,
    V1MessagesCountTokensRequest, V1MessagesRequest, build_tool_configuration,
    get_additional_model_request_fields,
};
use anthropic_response::EventConverter;
use anyhow::anyhow;
use async_trait::async_trait;
use aws_sdk_bedrockruntime::{
    Client,
    error::SdkError,
    operation::converse_stream::ConverseStreamError,
    primitives::event_stream::EventReceiver,
    types::{
        ContentBlock, ConverseStreamOutput, ConverseTokensRequest, CountTokensInput,
        Message as BedrockMessage, SystemContentBlock, TokenUsage,
        error::ConverseStreamOutputError,
    },
};
use aws_smithy_types::Document;
use axum::response::sse::Event;
use futures::stream::{BoxStream, StreamExt};
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::mpsc,
    time::{Instant, interval_at, timeout},
};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{error, info};
use uuid::Uuid;

use crate::bedrock::BedrockChatCompletion;

const PING_INTERVAL: Duration = Duration::from_secs(20);
const EVENT_TX_SEND_TIMEOUT: Duration = Duration::from_secs(30);
/// Synchronous window before falling to SSE + pings. Bedrock returns
/// validation/throttle/access errors in <200ms; this catches them so they
/// flow through `AppError` as proper HTTP 4xx with the upstream status code.
const CONNECT_ERROR_WINDOW: Duration = Duration::from_secs(15);

/// Maps a Bedrock `ConverseStreamError` to an Anthropic error `type` and
/// extracts the human-readable upstream message. Default arm falls back to
/// `api_error` so newly-introduced Bedrock variants never panic.
fn classify_bedrock_error(err: &SdkError<ConverseStreamError>) -> (&'static str, String) {
    let msg = err
        .as_service_error()
        .and_then(|e| e.meta().message())
        .map(String::from)
        .unwrap_or_else(|| err.to_string());

    let kind = match err.as_service_error() {
        Some(ConverseStreamError::ValidationException(_)) => "invalid_request_error",
        Some(ConverseStreamError::ThrottlingException(_)) => "rate_limit_error",
        Some(ConverseStreamError::AccessDeniedException(_)) => "permission_error",
        Some(ConverseStreamError::ServiceUnavailableException(_)) => "overloaded_error",
        Some(ConverseStreamError::ModelTimeoutException(_)) => "timeout_error",
        Some(ConverseStreamError::ModelStreamErrorException(_)) => "api_error",
        Some(ConverseStreamError::InternalServerException(_)) => "api_error",
        _ => "api_error",
    };
    (kind, msg)
}

/// Builds a single-line Anthropic-style SSE error frame. Used when the HTTP
/// status has already been committed as 200 and we can no longer surface the
/// failure as 4xx via `AppError`.
fn anthropic_error_event(kind: &str, message: &str) -> anyhow::Result<Event> {
    let payload = serde_json::json!({
        "type": "error",
        "error": { "type": kind, "message": message }
    });
    Ok(Event::default().event("error").data(payload.to_string()))
}

/// Sends a ping SSE event. Returns false if the consumer is gone or stuck.
async fn send_ping(event_tx: &mpsc::Sender<anyhow::Result<Event>>) -> bool {
    info!("Sending ping event");
    let ping_event = Ok(Event::default().event("ping").data(r#"{"type": "ping"}"#));
    match timeout(EVENT_TX_SEND_TIMEOUT, event_tx.send(ping_event)).await {
        Ok(Ok(())) => true,
        Ok(Err(_)) => {
            info!("SSE client disconnected, stopping Bedrock stream");
            false
        }
        Err(_) => {
            error!("Channel send timed out, consumer likely stuck");
            false
        }
    }
}

async fn process_bedrock_stream_events(
    mut stream: EventReceiver<ConverseStreamOutput, ConverseStreamOutputError>,
    model: String,
    usage_callback: Arc<dyn Fn(&TokenUsage) + Send + Sync>,
    event_tx: mpsc::Sender<anyhow::Result<Event>>,
    mut ping_interval: tokio::time::Interval,
) {
    let id = format!("msg_{}", Uuid::new_v4());
    let mut event_converter = EventConverter::new(id, model, usage_callback);
    loop {
        tokio::select! {
            biased;
            result = stream.recv() => {
                match result {
                    Ok(Some(output)) => {
                        if let Some(events) = event_converter.convert(&output) {
                            for (event_name, event) in events {
                                let sse_event = match serde_json::to_string(&event) {
                                    Ok(json) => Ok(Event::default().event(event_name).data(json)),
                                    Err(e) => Err(anyhow!("Failed to serialize event: {}", e)),
                                };
                                match timeout(EVENT_TX_SEND_TIMEOUT, event_tx.send(sse_event)).await {
                                    Ok(Ok(())) => {}
                                    Ok(Err(_)) => {
                                        info!("SSE client disconnected, stopping Bedrock stream");
                                        return;
                                    }
                                    Err(_) => {
                                        error!("Channel send timed out, consumer likely stuck");
                                        return;
                                    }
                                }
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        let event = anthropic_error_event(
                            "api_error",
                            &format!("Stream receive error: {e}"),
                        );
                        let _ = timeout(EVENT_TX_SEND_TIMEOUT, event_tx.send(event)).await;
                        break;
                    }
                }
            }
            _ = ping_interval.tick() => {
                if !send_ping(&event_tx).await {
                    return;
                }
            }
        }
    }
    info!("Bedrock stream finished");
}

#[async_trait]
pub trait V1MessagesProvider {
    async fn v1_messages_stream<F>(
        self,
        request: V1MessagesRequest,
        response_model_id: Option<String>,
        anthropic_beta: Option<Vec<String>>,
        usage_callback: F,
    ) -> anyhow::Result<BoxStream<'async_trait, anyhow::Result<Event>>>
    where
        F: Fn(&TokenUsage) + Send + Sync + 'static;

    async fn v1_messages_count_tokens(
        &self,
        request: &V1MessagesCountTokensRequest,
        inference_profile_prefixes: &[String],
    ) -> anyhow::Result<i32>;
}

fn log_v1_messages_request(request: &V1MessagesRequest) {
    match &request.messages {
        Messages::String(s) => {
            info!(
                "V1 Messages Request: single string message, len={}",
                s.len()
            );
        }
        Messages::Array(messages) => {
            for (i, message) in messages.iter().enumerate() {
                match message {
                    Message::User { content } => {
                        let user_content_types = match content {
                            UserContents::String(s) => format!("String(len={})", s.len()),
                            UserContents::Array(arr) => arr
                                .iter()
                                .map(|c| match c {
                                    UserContent::Document { .. } => "Document",
                                    UserContent::Image { .. } => "Image",
                                    UserContent::Text { .. } => "Text",
                                    UserContent::ToolResult { .. } => "ToolResult",
                                    UserContent::Thinking { .. } => "Thinking",
                                    UserContent::RedactedThinking { .. } => "RedactedThinking",
                                    UserContent::ServerToolResult { .. } => "ServerToolResult",
                                })
                                .collect::<Vec<_>>()
                                .join(", "),
                        };
                        info!(
                            "V1 Messages Request Message {}: role=user, content=[{}]",
                            i, user_content_types
                        );
                    }
                    Message::Assistant { content } => {
                        let assistant_content_types = match content {
                            AssistantContents::String(s) => format!("String(len={})", s.len()),
                            AssistantContents::Array(arr) => arr
                                .iter()
                                .map(|c| match c {
                                    AssistantContent::Text { .. } => "Text",
                                    AssistantContent::Thinking { .. } => "Thinking",
                                    AssistantContent::ToolUse { .. } => "ToolUse",
                                    AssistantContent::RedactedThinking { .. } => "RedactedThinking",
                                    AssistantContent::ServerToolUse { .. } => "ServerToolUse",
                                })
                                .collect::<Vec<_>>()
                                .join(", "),
                        };
                        info!(
                            "V1 Messages Request Message {}: role=assistant, content=[{}]",
                            i, assistant_content_types
                        );
                    }
                }
            }
        }
    }
}

fn log_bedrock_messages(messages: &[BedrockMessage]) {
    for (i, message) in messages.iter().enumerate() {
        let content_block_types = message
            .content()
            .iter()
            .map(|content_block| match content_block {
                ContentBlock::Document(_) => "Document",
                ContentBlock::GuardContent(_) => "GuardContent",
                ContentBlock::Image(_) => "Image",
                ContentBlock::ReasoningContent(_) => "ReasoningContent",
                ContentBlock::Text(_) => "Text",
                ContentBlock::ToolResult(_) => "ToolResult",
                ContentBlock::ToolUse(_) => "ToolUse",
                _ => "Unknown",
            })
            .collect::<Vec<_>>()
            .join(", ");
        info!(
            "Bedrock Message {}: role={:?}, content=[{}]",
            i,
            message.role(),
            content_block_types
        );
    }
}

pub struct BedrockV1MessagesProvider {
    bedrockruntime_client: Client,
}

/// Returns true if this is the Bedrock "thinking block modified" validation error.
fn is_thinking_block_modified_error(err: &SdkError<ConverseStreamError>) -> bool {
    let Some(message) = err.as_service_error().and_then(|e| e.meta().message()) else {
        return false;
    };

    message.contains(
        "`thinking` or `redacted_thinking` blocks in the latest assistant message cannot be modified",
    )
}

impl BedrockV1MessagesProvider {
    pub fn new(bedrockruntime_client: Client) -> Self {
        Self {
            bedrockruntime_client,
        }
    }
}

#[async_trait]
impl V1MessagesProvider for BedrockV1MessagesProvider {
    async fn v1_messages_stream<F>(
        self,
        request: V1MessagesRequest,
        response_model_id: Option<String>,
        anthropic_beta: Option<Vec<String>>,
        usage_callback: F,
    ) -> anyhow::Result<BoxStream<'async_trait, anyhow::Result<Event>>>
    where
        F: Fn(&TokenUsage) + Send + Sync + 'static,
    {
        let model = response_model_id.unwrap_or(request.model.clone());
        log_v1_messages_request(&request);
        let bedrock_chat_completion = BedrockChatCompletion::try_from(&request)?;
        let additional_model_request_fields = get_additional_model_request_fields(
            request.thinking.as_ref(),
            request.output_config.as_ref(),
            anthropic_beta.as_deref(),
            request.context_management.as_ref(),
        );
        if let Some(messages) = &bedrock_chat_completion.messages {
            log_bedrock_messages(messages);
        }
        info!(
            "Processed Anthropic request to Bedrock format with {} messages",
            bedrock_chat_completion
                .messages
                .as_ref()
                .map_or(0, |m| m.len())
        );

        info!(
            "Sending Anthropic request to Bedrock API for model: {}",
            bedrock_chat_completion.model_id
        );

        let (event_tx, event_rx) = mpsc::channel::<anyhow::Result<Event>>(1);
        let usage_callback = Arc::new(usage_callback);
        let client = self.bedrockruntime_client;

        // Race the connect against a short window: errors caught here flow
        // through `AppError` as proper HTTP 4xx with the upstream Bedrock
        // status. Slower connects fall to a 200 SSE response with pings.
        let mut send_fut = Box::pin(
            client
                .converse_stream()
                .model_id(bedrock_chat_completion.model_id)
                .set_system(bedrock_chat_completion.system_content_blocks)
                .set_messages(bedrock_chat_completion.messages)
                .set_tool_config(bedrock_chat_completion.tool_config)
                .set_inference_config(Some(bedrock_chat_completion.inference_config))
                .set_additional_model_request_fields(additional_model_request_fields.clone())
                .set_output_config(bedrock_chat_completion.output_config)
                .send(),
        );

        match timeout(CONNECT_ERROR_WINDOW, &mut send_fut).await {
            Ok(Ok(response)) => {
                info!("Successfully connected to Bedrock stream for Anthropic format");
                let ping_interval = interval_at(Instant::now() + PING_INTERVAL, PING_INTERVAL);
                tokio::spawn(process_bedrock_stream_events(
                    response.stream,
                    model,
                    usage_callback,
                    event_tx,
                    ping_interval,
                ));
            }
            Ok(Err(e)) if is_thinking_block_modified_error(&e) => {
                info!("Thinking block was modified; retrying with prior thinking text blanked");
                let mut retry_request = request;
                retry_request.blank_assistant_thinking_text();
                let retry_bcc = BedrockChatCompletion::try_from(&retry_request)?;
                let mut retry_fut = Box::pin(
                    client
                        .converse_stream()
                        .model_id(retry_bcc.model_id)
                        .set_system(retry_bcc.system_content_blocks)
                        .set_messages(retry_bcc.messages)
                        .set_tool_config(retry_bcc.tool_config)
                        .set_inference_config(Some(retry_bcc.inference_config))
                        .set_additional_model_request_fields(additional_model_request_fields)
                        .set_output_config(retry_bcc.output_config)
                        .send(),
                );
                match timeout(CONNECT_ERROR_WINDOW, &mut retry_fut).await {
                    Ok(Ok(response)) => {
                        info!("Retry succeeded with prior thinking text blanked");
                        let ping_interval =
                            interval_at(Instant::now() + PING_INTERVAL, PING_INTERVAL);
                        tokio::spawn(process_bedrock_stream_events(
                            response.stream,
                            model,
                            usage_callback,
                            event_tx,
                            ping_interval,
                        ));
                    }
                    Ok(Err(e)) => {
                        error!("Bedrock API error on retry: {:?}", e);
                        return Err(e.into());
                    }
                    Err(_) => {
                        tokio::spawn(async move {
                            let mut ping_interval =
                                interval_at(Instant::now() + PING_INTERVAL, PING_INTERVAL);
                            let result = loop {
                                tokio::select! {
                                    biased;
                                    r = &mut retry_fut => break r,
                                    _ = ping_interval.tick() => {
                                        if !send_ping(&event_tx).await { return; }
                                    }
                                }
                            };
                            match result {
                                Ok(response) => {
                                    process_bedrock_stream_events(
                                        response.stream,
                                        model,
                                        usage_callback,
                                        event_tx,
                                        ping_interval,
                                    )
                                    .await;
                                }
                                Err(e) => {
                                    error!(
                                        "Bedrock API error on retry after connect window: {:?}",
                                        e
                                    );
                                    let (kind, msg) = classify_bedrock_error(&e);
                                    let _ = timeout(
                                        EVENT_TX_SEND_TIMEOUT,
                                        event_tx.send(anthropic_error_event(kind, &msg)),
                                    )
                                    .await;
                                }
                            }
                        });
                    }
                }
            }
            Ok(Err(e)) => {
                error!("Bedrock API error: {:?}", e);
                return Err(e.into());
            }
            Err(_) => {
                tokio::spawn(async move {
                    let mut ping_interval =
                        interval_at(Instant::now() + PING_INTERVAL, PING_INTERVAL);
                    let result = loop {
                        tokio::select! {
                            biased;
                            r = &mut send_fut => break r,
                            _ = ping_interval.tick() => {
                                if !send_ping(&event_tx).await { return; }
                            }
                        }
                    };
                    match result {
                        Ok(response) => {
                            process_bedrock_stream_events(
                                response.stream,
                                model,
                                usage_callback,
                                event_tx,
                                ping_interval,
                            )
                            .await;
                        }
                        Err(e) => {
                            error!("Bedrock API error after connect window: {:?}", e);
                            let (kind, msg) = classify_bedrock_error(&e);
                            let _ = timeout(
                                EVENT_TX_SEND_TIMEOUT,
                                event_tx.send(anthropic_error_event(kind, &msg)),
                            )
                            .await;
                        }
                    }
                });
            }
        }

        Ok(ReceiverStream::new(event_rx).boxed())
    }

    async fn v1_messages_count_tokens(
        &self,
        request: &V1MessagesCountTokensRequest,
        inference_profile_prefixes: &[String],
    ) -> anyhow::Result<i32> {
        let messages: Option<Vec<BedrockMessage>> = Option::try_from(&request.messages)?;

        let system: Option<Vec<SystemContentBlock>> = request
            .system
            .as_ref()
            .map(Vec::<SystemContentBlock>::try_from)
            .transpose()?;

        let tool_config = request
            .tools
            .as_deref()
            .map(|tools| build_tool_configuration(tools, request.tool_choice.as_ref()))
            .transpose()?
            .flatten();

        let additional_model_request_fields = request.thinking.as_ref().map(Document::from);

        let converse_tokens_request = ConverseTokensRequest::builder()
            .set_additional_model_request_fields(additional_model_request_fields)
            .set_messages(messages)
            .set_system(system)
            .set_tool_config(tool_config)
            .build();

        let count_tokens_input = CountTokensInput::Converse(converse_tokens_request);

        let model_id = inference_profile_prefixes
            .iter()
            .find_map(|inference_profile_prefix| {
                request.model.strip_prefix(inference_profile_prefix)
            })
            .unwrap_or(&request.model);

        let result = self
            .bedrockruntime_client
            .count_tokens()
            .model_id(model_id)
            .input(count_tokens_input)
            .send()
            .await;

        match result {
            Ok(response) => Ok(response.input_tokens),
            Err(e) => {
                error!("Bedrock API error: {:?}", e);
                Err(e.into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_bedrockruntime::types::error::ValidationException;
    use aws_smithy_runtime_api::http::{
        Response as SmithyResponse, StatusCode as SmithyStatusCode,
    };
    use aws_smithy_types::body::SdkBody;
    use aws_smithy_types::error::ErrorMetadata;

    fn make_sdk_error(message: &str) -> SdkError<ConverseStreamError> {
        let raw = SmithyResponse::new(
            SmithyStatusCode::try_from(400).unwrap(),
            SdkBody::from("error"),
        );
        let err = ConverseStreamError::ValidationException(
            ValidationException::builder()
                .message(message)
                .meta(ErrorMetadata::builder().message(message).build())
                .build(),
        );
        SdkError::service_error(err, raw)
    }

    #[test]
    fn is_thinking_block_modified_error_returns_true_for_matching_error() {
        let err = make_sdk_error(
            "The model returned the following errors: messages.3.content.1: \
             `thinking` or `redacted_thinking` blocks in the latest assistant \
             message cannot be modified.",
        );
        assert!(is_thinking_block_modified_error(&err));
    }

    #[test]
    fn is_thinking_block_modified_error_returns_false_for_unrelated() {
        let err = make_sdk_error("Some other validation error");
        assert!(!is_thinking_block_modified_error(&err));
    }

    #[test]
    fn is_thinking_block_modified_error_returns_false_for_partial_message() {
        let err = make_sdk_error("thinking blocks cannot be modified but no message index here");
        assert!(!is_thinking_block_modified_error(&err));
    }

    #[test]
    fn is_thinking_block_modified_error_works_without_indices() {
        let err = make_sdk_error(
            "The model returned the following errors: \
             `thinking` or `redacted_thinking` blocks in the latest assistant \
             message cannot be modified.",
        );
        assert!(is_thinking_block_modified_error(&err));
    }
}
