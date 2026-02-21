use aws_smithy_types::Document;

use crate::{OutputConfig, Thinking, output_config};

pub fn get_additional_model_request_fields(
    thinking: Option<&Thinking>,
    output_config: Option<&OutputConfig>,
    anthropic_beta: Option<&[String]>,
) -> Option<Document> {
    let output_config_effort_document = match output_config {
        Some(OutputConfig::Effort { effort }) => {
            Some(output_config::get_output_config_effort_document(effort))
        }
        _ => None,
    };

    let anthropic_beta_document =
        anthropic_beta.and_then(output_config::get_anthropic_beta_document);

    [
        thinking.map(Document::from),
        output_config_effort_document,
        anthropic_beta_document,
    ]
    .into_iter()
    .flatten()
    .reduce(|mut a, b| {
        if let (Some(map_a), Document::Object(map_b)) = (a.as_object_mut(), b) {
            map_a.extend(map_b);
        }
        a
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OutputConfig, Thinking};

    #[test]
    fn get_additional_model_request_fields_returns_none_when_all_inputs_empty() {
        let result = get_additional_model_request_fields(None, None, None);
        assert!(result.is_none());
    }

    #[test]
    fn get_additional_model_request_fields_merges_thinking_effort_and_beta() {
        let thinking = Thinking::Enabled {
            budget_tokens: 1024,
        };
        let effort = OutputConfig::Effort {
            effort: "high".to_string(),
        };
        let beta = vec!["interleaved-thinking-2025-05-14".to_string()];

        let result =
            get_additional_model_request_fields(Some(&thinking), Some(&effort), Some(&beta));
        let Document::Object(map) = result.unwrap() else {
            panic!("expected Document::Object");
        };

        assert!(map.contains_key("thinking"));
        assert!(map.contains_key("output_config"));
        assert_eq!(
            map["anthropic_beta"],
            Document::Array(vec![Document::String(
                "interleaved-thinking-2025-05-14".to_string()
            )])
        );
    }
}
