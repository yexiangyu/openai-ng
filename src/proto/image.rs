use std::time::Duration;

use crate::error::*;
use http::{
    header::{self, HeaderValue},
    Method,
};
use reqwest::Body;
use smart_default::SmartDefault;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, SmartDefault)]
pub struct GenerationRequest {
    pub model: String,
    pub prompt: String,
    pub size: Option<String>,
    pub n: Option<i32>,
    pub response_format: Option<GenerationFormat>,
    pub seed: Option<i32>,
    pub steps: Option<i32>,
    pub cfg_scale: Option<f32>,
}

impl GenerationRequest {
    pub fn builder() -> GenerationRequestBuilder {
        GenerationRequestBuilder::default()
    }

    pub async fn call(
        &self,
        client: &crate::client::Client,
        timeout: Option<Duration>,
    ) -> Result<GenerationResponse> {
        let uri = "images/generations";

        let rep = client
            .call_impl(
                Method::POST,
                uri,
                vec![(
                    header::CONTENT_TYPE,
                    HeaderValue::from_str("application/json")?,
                )],
                Some(Body::from(serde_json::to_string(&self)?)),
                None,
                timeout,
            )
            .await?;

        let status = rep.status();

        let rep = serde_json::from_slice::<serde_json::Value>(rep.bytes().await?.as_ref())?;

        for l in serde_json::to_string_pretty(&rep)?.lines() {
            if status.is_client_error() || status.is_server_error() {
                tracing::error!("REP: {}", l);
            } else {
                tracing::trace!("REP: {}", l);
            }
        }

        if !status.is_success() {
            return Err(Error::ApiError(status.as_u16()));
        }

        Ok(serde_json::from_value(rep)?)
    }
}

#[derive(Debug, Clone, SmartDefault)]
pub struct GenerationRequestBuilder {
    model: Option<String>,
    prompt: Option<String>,
    size: Option<String>,
    n: Option<i32>,
    response_format: Option<GenerationFormat>,
    seed: Option<i32>,
    steps: Option<i32>,
    cfg_scale: Option<f32>,
}

impl GenerationRequestBuilder {
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    pub fn with_size(mut self, width: i32, height: i32) -> Self {
        self.size = Some(format!("{}x{}", width, height));
        self
    }

    pub fn with_n(mut self, n: i32) -> Self {
        self.n = Some(n);
        self
    }

    pub fn with_response_format(mut self, response_format: GenerationFormat) -> Self {
        self.response_format = Some(response_format);
        self
    }

    pub fn with_seed(mut self, seed: i32) -> Self {
        self.seed = Some(seed);
        self
    }

    pub fn with_steps(mut self, steps: i32) -> Self {
        self.steps = Some(steps);
        self
    }

    pub fn with_cfg_scale(mut self, cfg_scale: f32) -> Self {
        self.cfg_scale = Some(cfg_scale);
        self
    }

    pub fn build(self) -> Result<GenerationRequest> {
        let Self {
            model,
            prompt,
            size,
            n,
            response_format,
            seed,
            steps,
            cfg_scale,
        } = self;

        Ok(GenerationRequest {
            model: model.ok_or(Error::GenerationRequestBuild)?,
            prompt: prompt.ok_or(Error::GenerationRequestBuild)?,
            size,
            n,
            response_format,
            seed,
            steps,
            cfg_scale,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GenerationResponse {
    pub created: u64,
    pub data: Vec<GenerationData>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GenerationData {
    pub seed: i32,
    pub finish_reason: String,
    pub image: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(non_camel_case_types)]
pub enum GenerationFormat {
    b64_json,
    url,
}

#[cfg(test)]
#[tokio::test]
async fn test_genai_ok() -> Result<()> {
    use crate::client::Client;

    let client = Client::from_env_file(".env.stepfun.genai")?;
    let _ = tracing_subscriber::fmt::try_init();

    let model_name = std::env::var("OPENAI_API_MODEL_NAME")?;

    let res = GenerationRequest::builder()
        .with_prompt("Sweet and Sour Mandarin Fish, a chinese traitional dish.")
        .with_model(model_name)
        .build()?
        .call(&client, None)
        .await?;

    for data in serde_json::to_string_pretty(&res)?.lines() {
        tracing::info!("REP: {}", data);
    }

    Ok(())
}
