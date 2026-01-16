# Anthropic v1 Messages API Support

## Overview

This update adds comprehensive support for the Anthropic Messages API v1 to the LLM proxy, allowing it to accept Anthropic-formatted requests and convert them to AWS Bedrock format while streaming responses back in Anthropic's format.

## What Was Added

### New Crates

1. **anthropic-request** (`/anthropic-request/`)
   - Defines all Anthropic request types including:
     - `AnthropicRequest` - Main request structure
     - `AnthropicMessage` - Message structure with role and content
     - `ContentBlock` - Various content types (text, image, document, tool_use, tool_result, thinking)
     - `Tool` and `ToolChoice` - Tool calling support
     - `SystemPrompt` - System message configuration
     - `ThinkingConfig` - Extended thinking support
   - Includes conversions to Bedrock types via `TryFrom` trait implementations
   - **Key feature**: Filters out thinking blocks when converting to Bedrock (they're output-only)

2. **anthropic-response** (`/anthropic-response/`)
   - Defines all Anthropic response streaming event types:
     - `StreamEvent` - All SSE event types
     - `Delta` - Content deltas (text, tool input, thinking, signature)
     - `ContentBlockStartData` - Block initialization events
     - `MessageStartData` and `MessageDeltaData` - Message lifecycle
     - `Usage` - Token usage tracking with cache support
   - Includes tests for thinking block serialization

### Updated Crates

1. **chat**
   - New `converters.rs` module with `TryFrom<&AnthropicRequest> for BedrockChatCompletion`
   - Updated `providers.rs`:
     - New `process_anthropic_stream()` function to handle Bedrock → Anthropic SSE conversion
     - New `AnthropicStreamProvider` trait
     - Implementation for `BedrockChatCompletionsProvider`
     - Proper handling of thinking blocks with signature generation
     - Comprehensive logging for debugging

2. **server**
   - New `handlers/` module structure:
     - `handlers/anthropic.rs` - `/v1/messages` endpoint handler
     - `handlers/openai.rs` - `/v1/chat/completions` endpoint handler (refactored from main.rs)
     - `handlers/mod.rs` - Module exports
   - Updated `main.rs` to use new handler structure
   - Enhanced `error.rs` with error logging

3. **request**
   - Fixed `tool.rs` line 157 to properly check for empty tools array

## Key Features

### Anthropic API Compatibility

- ✅ Full support for Anthropic Messages API v1 format
- ✅ Tool calling (function calling)
- ✅ Multi-modal content (text, images, documents)
- ✅ System prompts (string and block format)
- ✅ Extended thinking blocks (Bedrock reasoning content)
- ✅ Prompt caching support (cache_control)
- ✅ Streaming responses with proper SSE events

### Thinking Blocks Support

The implementation includes special handling for "thinking" blocks:
- Bedrock sends `ReasoningContent` which is converted to `thinking_delta` events
- Automatically synthesizes `content_block_start` events for thinking blocks
- Generates placeholder signatures (Bedrock doesn't provide them)
- Properly filters thinking blocks from request history (they shouldn't be sent back)

### Stream Event Mapping

Bedrock → Anthropic event mapping:
- `MessageStart` → `message_start`
- `ContentBlockStart` → `content_block_start` (with type: text/tool_use/thinking)
- `ContentBlockDelta` → `content_block_delta` (with text/input_json/thinking deltas)
- `ContentBlockStop` → `content_block_stop` (with signature_delta for thinking blocks)
- `MessageStop` + `Metadata` → `message_delta` + `message_stop`

## API Endpoints

### `/v1/messages` (Anthropic Messages API)

Accepts Anthropic-formatted requests:
```json
{
  "model": "us.anthropic.claude-3-5-sonnet-20241022-v2:0",
  "max_tokens": 1024,
  "messages": [
    {"role": "user", "content": "Hello!"}
  ],
  "stream": true
}
```

Returns Anthropic-formatted SSE stream.

### `/v1/chat/completions` (OpenAI Chat Completions API)

Accepts OpenAI-formatted requests (existing functionality, now refactored).

## Testing

All tests pass:
- ✅ 9 tests in `anthropic-request` (message serialization, thinking block filtering)
- ✅ 3 tests in `anthropic-response` (thinking/signature delta serialization)
- ✅ Release build successful

## Build

```bash
cargo build --release
cargo test --release
```

Both commands complete successfully with no errors.

## Dependencies Added

- `base64 = "0.22"` - Image encoding/decoding
- `serde = { version = "1.0", features = ["derive"] }` - Serialization
- `anthropic-request` and `anthropic-response` workspace crates

## Important Implementation Notes

1. **Thinking Blocks Are Filtered**: When converting Anthropic messages to Bedrock format, thinking blocks are filtered out because they represent the model's internal reasoning and should not be sent back in the conversation history.

2. **Signature Generation**: Bedrock doesn't provide signatures for thinking blocks like Anthropic does, so the proxy generates placeholder signatures using UUIDs.

3. **Usage Tracking**: Usage information comes from Bedrock's `Metadata` events, not from `MessageStart`. The implementation buffers `MessageStop` events until `Metadata` arrives with usage info.

4. **Error Handling**: Enhanced error logging throughout the proxy for better debugging.

5. **SSE Keep-Alive**: Both endpoints use 15-second keep-alive intervals to prevent connection timeouts.

## Next Steps

To use this proxy:
1. Configure AWS credentials for Bedrock access
2. Update `config.toml` with host/port settings
3. Run the server: `cargo run --release`
4. Send requests to `/v1/messages` (Anthropic) or `/v1/chat/completions` (OpenAI)
