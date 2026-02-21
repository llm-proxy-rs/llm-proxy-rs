use axum::http::HeaderMap;
use tracing::warn;

pub fn filter_anthropic_beta(headers: &HeaderMap, whitelist: &[String]) -> Option<Vec<String>> {
    let requested: Vec<&str> = headers
        .get_all("anthropic-beta")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(|s| s.trim())
        .collect();

    let filtered_out: Vec<&str> = requested
        .iter()
        .filter(|r| !whitelist.iter().any(|b| b.as_str() == **r))
        .copied()
        .collect();

    if !filtered_out.is_empty() {
        warn!("anthropic_beta filtered out: {:?}", filtered_out);
    }

    let v: Vec<String> = whitelist
        .iter()
        .filter(|b| requested.contains(&b.as_str()))
        .cloned()
        .collect();

    if v.is_empty() { None } else { Some(v) }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_anthropic_beta_only_whitelisted_pass_through() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "anthropic-beta",
            "context-1m-2025-08-07,prompt-caching-scope-2026-01-05,effort-2025-11-24"
                .parse()
                .unwrap(),
        );
        let whitelist = vec![
            "context-1m-2025-08-07".to_string(),
            "effort-2025-11-24".to_string(),
        ];
        let result = filter_anthropic_beta(&headers, &whitelist);
        assert_eq!(
            result.unwrap(),
            vec![
                "context-1m-2025-08-07".to_string(),
                "effort-2025-11-24".to_string(),
            ]
        );
    }

    #[test]
    fn filter_anthropic_beta_non_whitelisted_all_filtered_out() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "anthropic-beta",
            "prompt-caching-scope-2026-01-05".parse().unwrap(),
        );
        let whitelist = vec!["effort-2025-11-24".to_string()];
        let result = filter_anthropic_beta(&headers, &whitelist);
        assert!(result.is_none());
    }

    #[test]
    fn filter_anthropic_beta_no_header_returns_none() {
        let headers = HeaderMap::new();
        let whitelist = vec![
            "context-1m-2025-08-07".to_string(),
            "effort-2025-11-24".to_string(),
        ];
        let result = filter_anthropic_beta(&headers, &whitelist);
        assert!(result.is_none());
    }
}
