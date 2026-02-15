use anyhow::{Context, bail};
use aws_sdk_bedrockruntime::types::{ImageBlock, ImageFormat, ImageSource};
use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct ImageUrl {
    pub url: String,
}

impl TryFrom<&ImageUrl> for ImageBlock {
    type Error = anyhow::Error;

    fn try_from(image_url: &ImageUrl) -> Result<Self, Self::Error> {
        let url = image_url.url.as_str();

        let (prefix, base64_data) = url
            .split_once(',')
            .context("Invalid image URL: missing comma separator")?;

        let format = match prefix {
            "data:image/jpeg;base64" => ImageFormat::Jpeg,
            "data:image/png;base64" => ImageFormat::Png,
            "data:image/gif;base64" => ImageFormat::Gif,
            "data:image/webp;base64" => ImageFormat::Webp,
            _ => bail!("Unsupported image URL prefix: {prefix}"),
        };

        let image_bytes = general_purpose::STANDARD.decode(base64_data)?;

        Ok(ImageBlock::builder()
            .format(format)
            .source(ImageSource::Bytes(image_bytes.into()))
            .build()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_without_comma_returns_error() {
        let image_url = ImageUrl {
            url: "not-a-data-url".into(),
        };
        assert!(ImageBlock::try_from(&image_url).is_err());
    }

    #[test]
    fn unsupported_prefix_returns_error() {
        let image_url = ImageUrl {
            url: "data:image/bmp;base64,AAAA".into(),
        };
        assert!(ImageBlock::try_from(&image_url).is_err());
    }
}
