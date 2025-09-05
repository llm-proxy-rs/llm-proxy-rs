pub struct Config {
    pub client: ClientConfig,
}

#[derive(Default)]
pub struct ClientConfig {
    pub base_url: String,
    pub max_tokens: Option<i32>,
    pub model_id: String,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
}
