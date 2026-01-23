use aws_smithy_types::Document;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Thinking {
    #[serde(rename = "type")]
    pub thinking_type: String,
    pub budget_tokens: i32,
}

impl From<&Thinking> for Document {
    fn from(thinking: &Thinking) -> Self {
        Document::Object(
            [(
                "thinking".to_string(),
                Document::Object(
                    [
                        (
                            "type".to_string(),
                            Document::String(thinking.thinking_type.clone()),
                        ),
                        (
                            "budget_tokens".to_string(),
                            Document::from(thinking.budget_tokens),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
            )]
            .into_iter()
            .collect(),
        )
    }
}
