use aws_sdk_bedrockruntime::types::{ImageBlock, ImageFormat, ImageSource};
use base64::{Engine as _, engine::general_purpose};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct ImageUrl {
    pub url: String,
}

impl From<&ImageUrl> for Option<ImageBlock> {
    fn from(image_url: &ImageUrl) -> Self {
        let url = image_url.url.as_str();

        let (prefix, base64_data) = url.split_once(',')?;

        let format = match prefix {
            "data:image/jpeg;base64" => ImageFormat::Jpeg,
            "data:image/png;base64" => ImageFormat::Png,
            "data:image/gif;base64" => ImageFormat::Gif,
            "data:image/webp;base64" => ImageFormat::Webp,
            _ => return None,
        };

        let image_bytes = general_purpose::STANDARD.decode(base64_data).ok()?;

        ImageBlock::builder()
            .format(format)
            .source(ImageSource::Bytes(image_bytes.into()))
            .build()
            .ok()
    }
}
