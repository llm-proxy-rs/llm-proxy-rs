use aws_smithy_types::Document;

use crate::anthropic_beta::get_anthropic_beta_document;
use crate::output_config::get_output_config_effort_document;
use crate::{ContextManagement, OutputConfig, Thinking};

pub fn get_additional_model_request_fields(
    thinking: Option<&Thinking>,
    output_config: Option<&OutputConfig>,
    anthropic_beta: Option<&[String]>,
    context_management: Option<&ContextManagement>,
) -> Option<Document> {
    let output_config_effort_document = match output_config {
        Some(OutputConfig::Effort { effort }) => Some(get_output_config_effort_document(effort)),
        _ => None,
    };

    let anthropic_beta_document = anthropic_beta.and_then(get_anthropic_beta_document);
    let context_management_document = context_management.map(Document::from);

    [
        thinking.map(Document::from),
        output_config_effort_document,
        anthropic_beta_document,
        context_management_document,
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
    use crate::{ContextManagement, ContextManagementEdit, OutputConfig, Thinking};

    #[test]
    fn get_additional_model_request_fields_returns_none_when_all_inputs_empty() {
        let result = get_additional_model_request_fields(None, None, None, None);
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
            get_additional_model_request_fields(Some(&thinking), Some(&effort), Some(&beta), None);
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

    #[test]
    fn get_additional_model_request_fields_includes_context_management() {
        let context_management = ContextManagement {
            edits: vec![ContextManagementEdit {
                edit_type: "clear_thinking_20251015".to_string(),
                keep: "all".to_string(),
            }],
        };

        let result =
            get_additional_model_request_fields(None, None, None, Some(&context_management))
                .expect("expected document");
        let Document::Object(map) = result else {
            panic!("expected Document::Object");
        };
        let Some(Document::Object(context_management_map)) = map.get("context_management") else {
            panic!("expected context_management object");
        };
        let Some(Document::Array(edits)) = context_management_map.get("edits") else {
            panic!("expected edits array");
        };
        assert_eq!(edits.len(), 1);
    }
}
