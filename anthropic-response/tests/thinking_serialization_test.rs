use anthropic_response::{ContentBlockStartData, Delta, StreamEvent};

#[test]
fn test_thinking_content_block_start_serialization() {
    let thinking_start = StreamEvent::ContentBlockStart {
        index: 0,
        content_block: ContentBlockStartData::Thinking {
            thinking: String::new(),
        },
    };

    let json = serde_json::to_string(&thinking_start).unwrap();
    println!("Thinking ContentBlockStart JSON: {}", json);

    // Verify the JSON structure
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["type"], "content_block_start");
    assert_eq!(parsed["index"], 0);
    assert_eq!(parsed["content_block"]["type"], "thinking");
}

#[test]
fn test_thinking_delta_serialization() {
    let thinking_delta = StreamEvent::ContentBlockDelta {
        index: 0,
        delta: Delta::ThinkingDelta {
            thinking: "Let me think about this...".to_string(),
        },
    };

    let json = serde_json::to_string(&thinking_delta).unwrap();
    println!("Thinking Delta JSON: {}", json);

    // Verify the JSON structure
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["type"], "content_block_delta");
    assert_eq!(parsed["index"], 0);
    assert_eq!(parsed["delta"]["type"], "thinking_delta");
    assert_eq!(parsed["delta"]["thinking"], "Let me think about this...");
}

#[test]
fn test_signature_delta_serialization() {
    let signature_delta = StreamEvent::ContentBlockDelta {
        index: 0,
        delta: Delta::SignatureDelta {
            signature: "test_signature".to_string(),
        },
    };

    let json = serde_json::to_string(&signature_delta).unwrap();
    println!("Signature Delta JSON: {}", json);

    // Verify the JSON structure
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["type"], "content_block_delta");
    assert_eq!(parsed["index"], 0);
    assert_eq!(parsed["delta"]["type"], "signature_delta");
    assert_eq!(parsed["delta"]["signature"], "test_signature");
}
