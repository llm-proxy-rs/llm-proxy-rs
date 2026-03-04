use aws_sdk_bedrockruntime::types::TokenUsage;
use tracing::info;

pub fn log_token_usage(usage: &TokenUsage) {
    let mut usage_message = format!(
        "Usage: input_tokens: {}, output_tokens: {}, total_tokens: {}",
        usage.input_tokens, usage.output_tokens, usage.total_tokens
    );
    if let Some(t) = usage.cache_read_input_tokens {
        usage_message.push_str(&format!(", cache_read_input_tokens: {}", t));
    }
    if let Some(t) = usage.cache_write_input_tokens {
        usage_message.push_str(&format!(", cache_write_input_tokens: {}", t));
    }
    info!("{}", usage_message);
}
