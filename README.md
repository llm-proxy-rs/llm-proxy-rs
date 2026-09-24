<p align="center">
  <img src="docs/logo.png" alt="llm-proxy-rs" width="640">
</p>

# llm-proxy-rs

A LLM proxy written in Rust. Speaks Anthropic `/v1/messages` and OpenAI `/v1/responses` and forwards to Amazon Bedrock.

## Related

- [gateway](https://github.com/llm-proxy-rs/gateway) — Gateway provides API keys and Cognito login support.
- [cost](https://github.com/llm-proxy-rs/cost) — Cost dashboard that can export CSV.

## Install

```bash
git clone https://github.com/llm-proxy-rs/llm-proxy-rs
cd llm-proxy-rs
cargo run -p server
```

Listens on `0.0.0.0:3000` by default (`config.toml`). AWS credentials come from the usual SDK chain.

## License

MIT. See [LICENSE](LICENSE).
