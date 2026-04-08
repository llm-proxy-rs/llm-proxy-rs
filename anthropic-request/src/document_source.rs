use std::cell::Cell;

use anyhow::bail;
use aws_sdk_bedrockruntime::types::{
    DocumentBlock, DocumentFormat, DocumentSource as BedrockDocumentSource,
};
use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};

pub struct DocumentCounter(Cell<usize>);

impl DocumentCounter {
    pub fn new() -> Self {
        Self(Cell::new(0))
    }

    fn next_name(&self) -> String {
        let n = self.0.get();
        self.0.set(n + 1);
        format!("document_{n}")
    }
}

impl Default for DocumentCounter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum DocumentSource {
    #[serde(rename = "base64")]
    Base64 { media_type: String, data: String },
    #[serde(rename = "url")]
    Url { url: String },
    #[serde(rename = "text")]
    Text { media_type: String, data: String },
}

impl DocumentSource {
    pub fn to_document_block(&self, counter: &DocumentCounter) -> anyhow::Result<DocumentBlock> {
        let name = counter.next_name();

        match self {
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
                    _ => bail!("Unsupported base64 document media type: {media_type}"),
                };

                let bytes = general_purpose::STANDARD.decode(data)?;

                Ok(DocumentBlock::builder()
                    .format(format)
                    .name(name)
                    .source(BedrockDocumentSource::Bytes(bytes.into()))
                    .build()?)
            }
            DocumentSource::Url { url } => bail!("URL document sources are not supported: {url}"),
            DocumentSource::Text { media_type, data } => {
                let format = match media_type.as_str() {
                    "text/csv" => DocumentFormat::Csv,
                    "text/html" => DocumentFormat::Html,
                    "text/markdown" => DocumentFormat::Md,
                    "text/plain" => DocumentFormat::Txt,
                    _ => bail!("Unsupported text document media type: {media_type}"),
                };

                Ok(DocumentBlock::builder()
                    .format(format)
                    .name(name)
                    .source(BedrockDocumentSource::Text(data.clone()))
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
        let counter = DocumentCounter::new();
        let source = DocumentSource::Base64 {
            media_type: "application/zip".into(),
            data: "".into(),
        };
        assert!(source.to_document_block(&counter).is_err());
    }

    #[test]
    fn invalid_base64_returns_error() {
        let counter = DocumentCounter::new();
        let source = DocumentSource::Base64 {
            media_type: "application/pdf".into(),
            data: "!!!not-base64!!!".into(),
        };
        assert!(source.to_document_block(&counter).is_err());
    }

    #[test]
    fn valid_pdf_produces_document_block() {
        let counter = DocumentCounter::new();
        let data = general_purpose::STANDARD.encode(b"%PDF-1.4");
        let source = DocumentSource::Base64 {
            media_type: "application/pdf".into(),
            data,
        };
        let block = source.to_document_block(&counter).unwrap();
        assert_eq!(*block.format(), DocumentFormat::Pdf);
    }

    #[test]
    fn counter_starts_at_zero_and_increments() {
        let counter = DocumentCounter::new();
        let data = general_purpose::STANDARD.encode(b"%PDF-1.4");
        let source = DocumentSource::Base64 {
            media_type: "application/pdf".into(),
            data,
        };
        let block0 = source.to_document_block(&counter).unwrap();
        let block1 = source.to_document_block(&counter).unwrap();
        assert_eq!(block0.name(), "document_0");
        assert_eq!(block1.name(), "document_1");
    }

    #[test]
    fn separate_counters_both_start_at_zero() {
        let counter1 = DocumentCounter::new();
        let counter2 = DocumentCounter::new();
        let data = general_purpose::STANDARD.encode(b"%PDF-1.4");
        let source = DocumentSource::Base64 {
            media_type: "application/pdf".into(),
            data,
        };
        let block_a = source.to_document_block(&counter1).unwrap();
        let block_b = source.to_document_block(&counter2).unwrap();
        assert_eq!(block_a.name(), "document_0");
        assert_eq!(block_b.name(), "document_0");
    }
}
