use async_trait::async_trait;
use aws_credential_types::provider::SharedCredentialsProvider;
use aws_sdk_bedrockruntime::types::TokenUsage;
use axum::body::Bytes;
use futures::Stream;
use futures::stream::{BoxStream, StreamExt};
use serde_json::Value;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::{
    sync::mpsc,
    time::{Instant, interval_at, timeout},
};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{error, info};

use crate::bedrock::mantle;

const PING_INTERVAL: Duration = Duration::from_secs(20);
const EVENT_TX_SEND_TIMEOUT: Duration = Duration::from_secs(30);
const PING_FRAME: &[u8] = b"event: ping\ndata: {\"type\": \"ping\"}\n\n";

pub struct V1ResponsesUpstream {
    pub status: reqwest::StatusCode,
    pub content_type: String,
    pub body: BoxStream<'static, Result<Bytes, reqwest::Error>>,
}

#[async_trait]
pub trait V1ResponsesProvider {
    async fn v1_responses_stream<F>(
        self,
        body: Vec<u8>,
        project: Option<&str>,
        usage_callback: F,
    ) -> anyhow::Result<V1ResponsesUpstream>
    where
        F: Fn(&TokenUsage) + Send + Sync + 'static;
}

pub struct MantleV1ResponsesProvider {
    http_client: reqwest::Client,
    region: String,
    credentials_provider: SharedCredentialsProvider,
}

impl MantleV1ResponsesProvider {
    pub fn new(
        http_client: reqwest::Client,
        region: String,
        credentials_provider: SharedCredentialsProvider,
    ) -> Self {
        Self {
            http_client,
            region,
            credentials_provider,
        }
    }
}

#[async_trait]
impl V1ResponsesProvider for MantleV1ResponsesProvider {
    async fn v1_responses_stream<F>(
        self,
        body: Vec<u8>,
        project: Option<&str>,
        usage_callback: F,
    ) -> anyhow::Result<V1ResponsesUpstream>
    where
        F: Fn(&TokenUsage) + Send + Sync + 'static,
    {
        let upstream = mantle::v1_responses_stream(
            &self.http_client,
            &self.credentials_provider,
            &self.region,
            body,
            project,
        )
        .await?;

        let status = upstream.status();
        let content_type = upstream
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/json")
            .to_string();
        let mode = UsageScanMode::from_content_type(&content_type);
        let is_sse = matches!(mode, UsageScanMode::Sse);
        let scanned = tap_responses_usage(upstream.bytes_stream(), mode, usage_callback);
        let body = if is_sse {
            with_sse_pings(scanned).boxed()
        } else {
            scanned.boxed()
        };

        Ok(V1ResponsesUpstream {
            status,
            content_type,
            body,
        })
    }
}

#[derive(Debug)]
enum UsageScanMode {
    Sse,
    Json,
}

impl UsageScanMode {
    fn from_content_type(content_type: &str) -> Self {
        if content_type
            .to_ascii_lowercase()
            .contains("text/event-stream")
        {
            Self::Sse
        } else {
            Self::Json
        }
    }
}

struct UsageScanner {
    mode: UsageScanMode,
    buf: Vec<u8>,
    fired: bool,
}

impl UsageScanner {
    fn new(mode: UsageScanMode) -> Self {
        Self {
            mode,
            buf: Vec::new(),
            fired: false,
        }
    }

    fn feed(&mut self, chunk: &[u8], callback: &dyn Fn(&TokenUsage)) {
        if self.fired {
            return;
        }
        match self.mode {
            UsageScanMode::Sse => {
                self.buf.extend_from_slice(chunk);
                while let Some(i) = self.buf.iter().position(|&b| b == b'\n') {
                    let mut line: Vec<u8> = self.buf.drain(..=i).collect();
                    if line.last() == Some(&b'\n') {
                        line.pop();
                    }
                    if line.last() == Some(&b'\r') {
                        line.pop();
                    }
                    self.handle_sse_line(&line, callback);
                    if self.fired {
                        return;
                    }
                }
            }
            UsageScanMode::Json => {
                self.buf.extend_from_slice(chunk);
            }
        }
    }

    fn finish(&mut self, callback: &dyn Fn(&TokenUsage)) {
        if self.fired {
            return;
        }
        match self.mode {
            UsageScanMode::Sse => {
                if !self.buf.is_empty() {
                    let line = std::mem::take(&mut self.buf);
                    self.handle_sse_line(&line, callback);
                }
            }
            UsageScanMode::Json => {
                if let Ok(value) = serde_json::from_slice::<Value>(&self.buf)
                    && let Some(usage) = value.get("usage").and_then(token_usage_from_openai)
                {
                    self.fired = true;
                    callback(&usage);
                }
            }
        }
    }

    fn handle_sse_line(&mut self, line: &[u8], callback: &dyn Fn(&TokenUsage)) {
        let Ok(line) = std::str::from_utf8(line) else {
            return;
        };
        let Some(data) = line.strip_prefix("data:") else {
            return;
        };
        let data = data.trim_start();
        if data.is_empty() || data == "[DONE]" {
            return;
        }
        let Ok(value) = serde_json::from_str::<Value>(data) else {
            return;
        };
        if value.get("type").and_then(|t| t.as_str()) != Some("response.completed") {
            return;
        }
        if let Some(usage) = value
            .pointer("/response/usage")
            .and_then(token_usage_from_openai)
        {
            self.fired = true;
            callback(&usage);
        }
    }
}

fn i32_from_json(v: &Value) -> Option<i32> {
    i32::try_from(v.as_i64()?).ok()
}

fn token_usage_from_openai(usage: &Value) -> Option<TokenUsage> {
    let input_tokens = usage.get("input_tokens").and_then(i32_from_json)?;
    let output_tokens = usage.get("output_tokens").and_then(i32_from_json)?;
    let total_tokens = usage
        .get("total_tokens")
        .and_then(i32_from_json)
        .unwrap_or(input_tokens.saturating_add(output_tokens));
    let cache_read_input_tokens = usage
        .pointer("/input_tokens_details/cached_tokens")
        .and_then(i32_from_json);

    TokenUsage::builder()
        .input_tokens(input_tokens)
        .output_tokens(output_tokens)
        .total_tokens(total_tokens)
        .set_cache_read_input_tokens(cache_read_input_tokens)
        .build()
        .ok()
}

fn tap_responses_usage<S, F>(
    stream: S,
    mode: UsageScanMode,
    usage_callback: F,
) -> impl Stream<Item = Result<Bytes, reqwest::Error>> + Send
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
    F: Fn(&TokenUsage) + Send + Sync + 'static,
{
    let callback = Arc::new(usage_callback);
    let mut stream = Box::pin(stream.fuse());
    let mut scanner = UsageScanner::new(mode);
    let mut done = false;

    futures::stream::poll_fn(move |cx: &mut Context<'_>| {
        if done {
            return Poll::Ready(None);
        }
        match stream.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(chunk))) => {
                scanner.feed(&chunk, &*callback);
                Poll::Ready(Some(Ok(chunk)))
            }
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => {
                done = true;
                scanner.finish(&*callback);
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    })
}

/// Injects Anthropic-style SSE ping frames while the upstream Responses stream
/// is idle, so proxies/clients do not drop the connection.
fn with_sse_pings<S>(stream: S) -> impl Stream<Item = Result<Bytes, reqwest::Error>> + Send
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Send + 'static,
{
    let (tx, rx) = mpsc::channel(16);
    tokio::spawn(async move {
        let mut stream = Box::pin(stream.fuse());
        let mut ping_interval = interval_at(Instant::now() + PING_INTERVAL, PING_INTERVAL);
        loop {
            tokio::select! {
                biased;
                next = stream.next() => {
                    match next {
                        Some(item) => {
                            if !send_chunk(&tx, item).await {
                                return;
                            }
                        }
                        None => return,
                    }
                }
                _ = ping_interval.tick() => {
                    info!("Sending ping event");
                    if !send_chunk(&tx, Ok(Bytes::from_static(PING_FRAME))).await {
                        return;
                    }
                }
            }
        }
    });
    ReceiverStream::new(rx)
}

/// Forwards a chunk to the consumer. Returns false if the consumer is gone or stuck.
async fn send_chunk(
    tx: &mpsc::Sender<Result<Bytes, reqwest::Error>>,
    item: Result<Bytes, reqwest::Error>,
) -> bool {
    match timeout(EVENT_TX_SEND_TIMEOUT, tx.send(item)).await {
        Ok(Ok(())) => true,
        Ok(Err(_)) => {
            info!("SSE client disconnected, stopping Responses stream");
            false
        }
        Err(_) => {
            error!("Channel send timed out, consumer likely stuck");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn collect_usage(chunks: &[&[u8]], mode: UsageScanMode) -> Option<TokenUsage> {
        let captured = Arc::new(Mutex::new(None));
        let captured_cb = captured.clone();
        let mut scanner = UsageScanner::new(mode);
        let callback = move |usage: &TokenUsage| {
            *captured_cb.lock().unwrap() = Some(usage.clone());
        };
        for chunk in chunks {
            scanner.feed(chunk, &callback);
        }
        scanner.finish(&callback);
        captured.lock().unwrap().clone()
    }

    #[test]
    fn parses_usage_from_response_completed_sse() {
        let event = concat!(
            "event: response.completed\n",
            r#"data: {"type":"response.completed","response":{"usage":{"input_tokens":8,"output_tokens":4,"total_tokens":12,"input_tokens_details":{"cached_tokens":3}}}}"#,
            "\n\n",
        );
        let usage = collect_usage(&[event.as_bytes()], UsageScanMode::Sse).expect("usage");
        assert_eq!(usage.input_tokens, 8);
        assert_eq!(usage.output_tokens, 4);
        assert_eq!(usage.total_tokens, 12);
        assert_eq!(usage.cache_read_input_tokens, Some(3));
    }

    #[test]
    fn parses_usage_across_chunk_boundaries() {
        let part1 = b"data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":1,\"output_tokens\":2,\"total_tokens\":";
        let part2 = b"3}}}\n";
        let usage = collect_usage(&[part1, part2], UsageScanMode::Sse).expect("usage");
        assert_eq!(usage.input_tokens, 1);
        assert_eq!(usage.output_tokens, 2);
        assert_eq!(usage.total_tokens, 3);
    }

    #[test]
    fn parses_usage_from_non_stream_json() {
        let body =
            br#"{"id":"resp_1","usage":{"input_tokens":10,"output_tokens":20,"total_tokens":30}}"#;
        let usage = collect_usage(&[body], UsageScanMode::Json).expect("usage");
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.output_tokens, 20);
        assert_eq!(usage.total_tokens, 30);
    }

    #[test]
    fn ignores_sse_events_without_completed_usage() {
        let event = concat!(
            "event: response.created\n",
            r#"data: {"type":"response.created","response":{"usage":null}}"#,
            "\n\n",
        );
        assert!(collect_usage(&[event.as_bytes()], UsageScanMode::Sse).is_none());
    }

    #[test]
    fn rejects_usage_outside_i32_range() {
        let body =
            br#"{"usage":{"input_tokens":3000000000,"output_tokens":1,"total_tokens":3000000001}}"#;
        assert!(collect_usage(&[body], UsageScanMode::Json).is_none());
    }

    #[test]
    fn falls_back_total_and_skips_out_of_range_cache() {
        let body = br#"{"usage":{"input_tokens":10,"output_tokens":20,"input_tokens_details":{"cached_tokens":3000000000}}}"#;
        let usage = collect_usage(&[body], UsageScanMode::Json).expect("usage");
        assert_eq!(usage.input_tokens, 10);
        assert_eq!(usage.output_tokens, 20);
        assert_eq!(usage.total_tokens, 30);
        assert_eq!(usage.cache_read_input_tokens, None);
    }

    #[test]
    fn ping_frame_matches_anthropic_keepalive_format() {
        assert_eq!(PING_FRAME, b"event: ping\ndata: {\"type\": \"ping\"}\n\n");
    }

    #[tokio::test(start_paused = true)]
    async fn injects_ping_while_upstream_idle() {
        let (upstream_tx, upstream_rx) = mpsc::channel::<Result<Bytes, reqwest::Error>>(4);
        let mut stream = Box::pin(with_sse_pings(ReceiverStream::new(upstream_rx)));

        // Let the relay task start and register its ping timer before advancing.
        tokio::task::yield_now().await;
        tokio::time::advance(PING_INTERVAL).await;

        let ping = stream
            .next()
            .await
            .expect("stream ended")
            .expect("ping errored");
        assert_eq!(ping.as_ref(), PING_FRAME);

        upstream_tx
            .send(Ok(Bytes::from_static(
                b"event: response.created\ndata: {}\n\n",
            )))
            .await
            .unwrap();
        let chunk = stream
            .next()
            .await
            .expect("stream ended")
            .expect("chunk errored");
        assert_eq!(chunk.as_ref(), b"event: response.created\ndata: {}\n\n");
    }
}
