use aws_smithy_types::Document;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct ContextManagement {
    pub edits: Vec<ContextManagementEdit>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ContextManagementEdit {
    #[serde(rename = "type")]
    pub edit_type: String,
    pub keep: Keep,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Keep {
    String(String),
    Number(u64),
}

impl From<&Keep> for Document {
    fn from(keep: &Keep) -> Self {
        match keep {
            Keep::String(s) => Document::String(s.clone()),
            Keep::Number(n) => Document::Object(
                [
                    (
                        "type".to_string(),
                        Document::String("thinking_turns".to_string()),
                    ),
                    (
                        "value".to_string(),
                        Document::Number(aws_smithy_types::Number::PosInt(*n)),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
        }
    }
}

impl From<&ContextManagement> for Document {
    fn from(context_management: &ContextManagement) -> Self {
        let edits = context_management
            .edits
            .iter()
            .map(|edit| {
                Document::Object(
                    [
                        ("type".to_string(), Document::String(edit.edit_type.clone())),
                        ("keep".to_string(), Document::from(&edit.keep)),
                    ]
                    .into_iter()
                    .collect(),
                )
            })
            .collect();

        Document::Object(
            [(
                "context_management".to_string(),
                Document::Object(
                    [("edits".to_string(), Document::Array(edits))]
                        .into_iter()
                        .collect(),
                ),
            )]
            .into_iter()
            .collect(),
        )
    }
}
