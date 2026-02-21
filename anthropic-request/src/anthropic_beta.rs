use aws_smithy_types::Document;

pub(crate) fn get_anthropic_beta_document(anthropic_beta: &[String]) -> Option<Document> {
    if anthropic_beta.is_empty() {
        None
    } else {
        Some(Document::Object(
            [(
                "anthropic_beta".to_string(),
                Document::Array(
                    anthropic_beta
                        .iter()
                        .map(|s| Document::String(s.clone()))
                        .collect(),
                ),
            )]
            .into_iter()
            .collect(),
        ))
    }
}
