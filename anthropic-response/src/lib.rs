use serde::{Deserialize, Serialize};

pub mod content_block_delta;
pub mod event;
pub mod message;
mod stream;

pub use content_block_delta::*;
pub use event::*;
pub use message::*;
pub use stream::*;

#[derive(Debug, Deserialize, Serialize)]
pub struct V1MessagesCountTokensResponse {
    pub input_tokens: i32,
}
