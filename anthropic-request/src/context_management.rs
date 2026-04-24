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
    pub keep: String,
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
                        ("keep".to_string(), Document::String(edit.keep.clone())),
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
