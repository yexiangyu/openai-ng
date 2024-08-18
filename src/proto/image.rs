use crate::error::*;
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
