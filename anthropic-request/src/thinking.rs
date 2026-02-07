use aws_smithy_types::Document;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Thinking {
    Enabled { budget_tokens: i32 },
    Adaptive,
}

impl From<&Thinking> for Document {
    fn from(thinking: &Thinking) -> Self {
        Document::Object(
            [(
                "thinking".to_string(),
                Document::Object(match thinking {
                    Thinking::Enabled { budget_tokens } => [
                        ("type".to_string(), Document::String("enabled".to_string())),
                        ("budget_tokens".to_string(), Document::from(*budget_tokens)),
                    ]
                    .into_iter()
                    .collect(),
                    Thinking::Adaptive => {
                        [("type".to_string(), Document::String("adaptive".to_string()))]
                            .into_iter()
                            .collect()
                    }
                }),
            )]
            .into_iter()
            .collect(),
        )
    }
}
