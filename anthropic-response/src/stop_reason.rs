use aws_sdk_bedrockruntime::types::StopReason;

/// Recovers Bedrock's omitted matched stop sequence when exactly one sequence
/// was configured on the request.
pub fn get_stop_sequence(
    stop_reason: &StopReason,
    request_stop_sequences: Option<&[String]>,
) -> Option<String> {
    if stop_reason == &StopReason::StopSequence {
        match request_stop_sequences {
            Some([only]) => Some(only.clone()),
            _ => None,
        }
    } else {
        None
    }
}
