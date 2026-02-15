use anyhow::bail;
use aws_sdk_bedrockruntime::types::{ImageBlock, ImageFormat, ImageSource as BedrockImageSource};
use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ImageSource {
    #[serde(rename = "base64")]
    Base64 { media_type: String, data: String },
}

impl TryFrom<&ImageSource> for ImageBlock {
    type Error = anyhow::Error;

    fn try_from(source: &ImageSource) -> Result<Self, Self::Error> {
        match source {
            ImageSource::Base64 { media_type, data } => {
                let format = match media_type.as_str() {
                    "image/gif" => ImageFormat::Gif,
                    "image/jpeg" => ImageFormat::Jpeg,
                    "image/png" => ImageFormat::Png,
                    "image/webp" => ImageFormat::Webp,
                    _ => bail!("Unsupported image media type: {media_type}"),
                };

                let bytes = general_purpose::STANDARD.decode(data)?;

                Ok(ImageBlock::builder()
                    .format(format)
                    .source(BedrockImageSource::Bytes(bytes.into()))
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
        let source = ImageSource::Base64 {
            media_type: "image/bmp".into(),
            data: "".into(),
        };
        assert!(ImageBlock::try_from(&source).is_err());
    }

    #[test]
    fn invalid_base64_returns_error() {
        let source = ImageSource::Base64 {
            media_type: "image/png".into(),
            data: "!!!not-base64!!!".into(),
        };
        assert!(ImageBlock::try_from(&source).is_err());
    }

    #[test]
    fn valid_png_produces_image_block() {
        let data = general_purpose::STANDARD.encode([0x89, 0x50, 0x4E, 0x47]);
        let source = ImageSource::Base64 {
            media_type: "image/png".into(),
            data,
        };
        let block = ImageBlock::try_from(&source).unwrap();
        assert_eq!(block.format, ImageFormat::Png);
    }
}
