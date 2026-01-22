pub fn value_to_document(value: &serde_json::Value) -> aws_smithy_types::Document {
    match value {
        serde_json::Value::Null => aws_smithy_types::Document::Null,
        serde_json::Value::Bool(b) => aws_smithy_types::Document::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                aws_smithy_types::Document::Number(if i >= 0 {
                    aws_smithy_types::Number::PosInt(i as u64)
                } else {
                    aws_smithy_types::Number::NegInt(i)
                })
            } else {
                aws_smithy_types::Document::Number(aws_smithy_types::Number::Float(
                    n.as_f64().unwrap_or(0.0),
                ))
            }
        }
        serde_json::Value::String(s) => aws_smithy_types::Document::String(s.clone()),
        serde_json::Value::Array(a) => {
            aws_smithy_types::Document::Array(a.iter().map(value_to_document).collect())
        }
        serde_json::Value::Object(o) => aws_smithy_types::Document::Object(
            o.iter()
                .map(|(k, v)| (k.clone(), value_to_document(v)))
                .collect(),
        ),
    }
}
