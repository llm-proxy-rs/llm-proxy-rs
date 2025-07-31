# Image Support for LLM Proxy

This proxy now supports image content in addition to text content, allowing you to send images to supported LLM models through AWS Bedrock.

## Supported Image Formats

The proxy supports the following image formats:
- PNG
- JPEG/JPG
- GIF
- WEBP

Images should be provided as base64-encoded data.

## API Usage

### Single Image Message

```json
{
  "model": "anthropic.claude-3-haiku-20240307-v1:0",
  "messages": [
    {
      "role": "user",
      "content": [
        {
          "type": "image",
          "image": {
            "format": "png",
            "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChAI9jU77yQAAAABJRU5ErkJggg=="
          }
        }
      ]
    }
  ]
}
```

### Mixed Text and Image Content

```json
{
  "model": "anthropic.claude-3-haiku-20240307-v1:0",
  "messages": [
    {
      "role": "user",
      "content": [
        {
          "type": "text",
          "text": "What do you see in this image?"
        },
        {
          "type": "image",
          "image": {
            "format": "jpeg",
            "data": "/9j/4AAQSkZJRgABAQEAYABgAAD..."
          }
        }
      ]
    }
  ]
}
```

### Tool Results with Images

Images can also be included in tool results:

```json
{
  "role": "tool",
  "tool_call_id": "call_123",
  "content": [
    {
      "type": "text",
      "text": "Here's the generated chart:"
    },
    {
      "type": "image",
      "image": {
        "format": "png",
        "data": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChAI9jU77yQAAAABJRU5ErkJggg=="
      }
    }
  ]
}
```

## Implementation Details

### Content Structure

The proxy now supports an `Image` variant in the `Content` enum:

```rust
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Content {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { image: ImageContent },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ImageContent {
    pub format: String,        // "png", "jpeg", "gif", "webp"
    pub data: String,          // base64 encoded image data
}
```

### AWS Bedrock Integration

Images are automatically converted to AWS Bedrock's `ImageBlock` format:
- Base64 data is decoded to bytes
- Format is mapped to AWS `ImageFormat` enum
- `ImageSource::Bytes` is used to provide the image data

### Error Handling

The proxy handles various error conditions gracefully:
- Invalid base64 data → converted to error text message
- Unsupported image format → defaults to PNG
- Images in system messages → converted to error text (not supported by Bedrock)

## Model Compatibility

Image support depends on the underlying model's capabilities. Currently supported models include:
- Anthropic Claude 3 models (Haiku, Sonnet, Opus)
- Other vision-capable models available through AWS Bedrock

## Testing

The implementation includes comprehensive tests to verify:
- Single image content conversion
- Mixed text and image content
- Tool result image handling
- Error conditions

Run tests with:
```bash
cargo test -p request test_image_support
```

## Example Base64 Images

For testing, here's a minimal 1x1 PNG image:
```
iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChAI9jU77yQAAAABJRU5ErkJggg==
```

This represents a transparent 1x1 pixel PNG image, useful for testing the image pipeline without large data. 