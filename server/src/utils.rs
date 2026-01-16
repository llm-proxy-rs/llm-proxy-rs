use aws_sdk_bedrockruntime::types::TokenUsage;
use tracing::info;

pub fn usage_callback(usage: &TokenUsage) {
    let mut msg = format!(
        "Usage: input_tokens: {}, output_tokens: {}, total_tokens: {}",
        usage.input_tokens, usage.output_tokens, usage.total_tokens
    );
    if let Some(t) = usage.cache_read_input_tokens {
        msg.push_str(&format!(", cache_read_input_tokens: {}", t));
    }
    if let Some(t) = usage.cache_write_input_tokens {
        msg.push_str(&format!(", cache_write_input_tokens: {}", t));
    }
    info!("{}", msg);
}
