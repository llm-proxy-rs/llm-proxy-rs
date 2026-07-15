use async_trait::async_trait;
use aws_credential_types::provider::SharedCredentialsProvider;

use crate::bedrock::mantle;

#[async_trait]
pub trait V1ResponsesProvider {
    async fn v1_responses_stream(
        self,
        body: Vec<u8>,
        project: Option<&str>,
    ) -> anyhow::Result<reqwest::Response>;
}

pub struct MantleV1ResponsesProvider {
    http_client: reqwest::Client,
    region: String,
    credentials_provider: SharedCredentialsProvider,
}

impl MantleV1ResponsesProvider {
    pub fn new(
        http_client: reqwest::Client,
        region: String,
        credentials_provider: SharedCredentialsProvider,
    ) -> Self {
        Self {
            http_client,
            region,
            credentials_provider,
        }
    }
}

#[async_trait]
impl V1ResponsesProvider for MantleV1ResponsesProvider {
    async fn v1_responses_stream(
        self,
        body: Vec<u8>,
        project: Option<&str>,
    ) -> anyhow::Result<reqwest::Response> {
        mantle::v1_responses_stream(
            &self.http_client,
            &self.credentials_provider,
            &self.region,
            body,
            project,
        )
        .await
    }
}
