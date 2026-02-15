use anyhow::bail;
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

impl TryFrom<&DocumentSource> for DocumentBlock {
    type Error = anyhow::Error;

    fn try_from(source: &DocumentSource) -> Result<Self, Self::Error> {
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
                    _ => bail!("Unsupported document media type: {media_type}"),
                };

                let bytes = general_purpose::STANDARD.decode(data)?;

                Ok(DocumentBlock::builder()
                    .format(format)
                    .name("document")
                    .source(BedrockDocumentSource::Bytes(bytes.into()))
                    .build()?)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_media_type_returns_error() {
        let source = DocumentSource::Base64 {
            media_type: "application/zip".into(),
            data: "".into(),
        };
        assert!(DocumentBlock::try_from(&source).is_err());
    }

    #[test]
    fn invalid_base64_returns_error() {
        let source = DocumentSource::Base64 {
            media_type: "application/pdf".into(),
            data: "!!!not-base64!!!".into(),
        };
        assert!(DocumentBlock::try_from(&source).is_err());
    }

    #[test]
    fn valid_pdf_produces_document_block() {
        let data = general_purpose::STANDARD.encode(b"%PDF-1.4");
        let source = DocumentSource::Base64 {
            media_type: "application/pdf".into(),
            data,
        };
        let block = DocumentBlock::try_from(&source).unwrap();
        assert_eq!(*block.format(), DocumentFormat::Pdf);
    }
}
