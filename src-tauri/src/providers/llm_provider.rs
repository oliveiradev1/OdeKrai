use super::{
    anthropic, chat_completions, perplexity, AuthRequest, ChatRequest, ProviderCapabilities,
};
use reqwest::{Client, Response, Url};
use serde_json::Value;

pub(crate) trait LlmProvider {
    fn capabilities(&self) -> ProviderCapabilities;
    fn request_body(&self, request: &ChatRequest) -> Value;

    async fn chat_stream(
        &self,
        client: &Client,
        endpoint: Url,
        provider_id: &str,
        credential: &str,
        request: &ChatRequest,
    ) -> Result<Response, String> {
        let mut builder = client
            .post(endpoint)
            .pipe_auth(provider_id, credential)?
            .json(&self.request_body(request));
        if provider_id == "anthropic" {
            builder = builder.header("anthropic-version", "2023-06-01");
        }
        builder
            .send()
            .await
            .map_err(|error| super::safe_transport_error(&error))
    }
}

pub(crate) struct AnthropicProvider;

impl LlmProvider for AnthropicProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            tools: true,
            vision: false,
            context_window_tokens: None,
            image_generation: false,
            verified: true,
        }
    }

    fn request_body(&self, request: &ChatRequest) -> Value {
        anthropic::chat_request_body(request)
    }
}

pub(crate) struct ChatCompletionsProvider {
    pub(crate) capabilities: ProviderCapabilities,
    pub(crate) include_usage: bool,
    pub(crate) reasoning_effort: Option<&'static str>,
}

impl LlmProvider for ChatCompletionsProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        self.capabilities
    }

    fn request_body(&self, request: &ChatRequest) -> Value {
        chat_completions::chat_request_body(request, self.include_usage, self.reasoning_effort)
    }
}

pub(crate) struct PerplexityProvider;

impl LlmProvider for PerplexityProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: false,
            tools: false,
            vision: false,
            context_window_tokens: None,
            image_generation: false,
            verified: false,
        }
    }

    fn request_body(&self, request: &ChatRequest) -> Value {
        perplexity::chat_request_body(request)
    }
}
