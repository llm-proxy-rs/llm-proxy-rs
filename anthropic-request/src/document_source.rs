use aws_sdk_bedrockruntime::types::{
    DocumentBlock, DocumentFormat, DocumentSource as BedrockDocumentSource,
};
use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum DocumentSource {
    #[serde(rename = "base64")]
    Base64 { media_type: String, data: String },
}

impl From<&DocumentSource> for Option<DocumentBlock> {
    fn from(source: &DocumentSource) -> Self {
        match source {
            DocumentSource::Base64 { media_type, data } => {
                let format = match media_type.as_str() {
                    "application/msword" => DocumentFormat::Doc,
                    "application/pdf" => DocumentFormat::Pdf,
                    "application/vnd.ms-excel" => DocumentFormat::Xls,
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
                        DocumentFormat::Xlsx
                    }
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
                        DocumentFormat::Docx
                    }
                    "text/csv" => DocumentFormat::Csv,
                    "text/html" => DocumentFormat::Html,
                    "text/markdown" => DocumentFormat::Md,
                    "text/plain" => DocumentFormat::Txt,
                    _ => return None,
                };

                let bytes = general_purpose::STANDARD.decode(data).ok()?;

                DocumentBlock::builder()
                    .format(format)
                    .name("document")
                    .source(BedrockDocumentSource::Bytes(bytes.into()))
                    .build()
                    .ok()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_media_type_returns_none() {
        let source = DocumentSource::Base64 {
            media_type: "application/zip".into(),
            data: "".into(),
        };
        assert!(Option::<DocumentBlock>::from(&source).is_none());
    }

    #[test]
    fn valid_pdf_produces_document_block() {
        let data = general_purpose::STANDARD.encode(b"%PDF-1.4");
        let source = DocumentSource::Base64 {
            media_type: "application/pdf".into(),
            data,
        };
        let block = Option::<DocumentBlock>::from(&source).unwrap();
        assert_eq!(*block.format(), DocumentFormat::Pdf);
    }
}
