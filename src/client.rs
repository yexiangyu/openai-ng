use crate::auth::*;
use crate::error::*;
use crate::proto::*;
use chat::ChatCompletionRequest;
use chat::ChatCompletionResponse;
use chat::ChatCompletionStreamData;
use http::HeaderName;
use http::HeaderValue;
use http::Method;
use image::GenerationRequest;
use image::GenerationResponse;
use reqwest::multipart::Form;
use reqwest::Body;
use reqwest::Response;
use smart_default::SmartDefault;
use std::time::Duration;
use sys::ModelListResponse;
use tokio::sync::mpsc::Receiver;
use tracing::*;
use url::Url;

#[derive(SmartDefault)]
pub struct ClientBuilder {
    pub base_url: Option<Url>,
    pub authenticator: Option<Box<dyn AuthenticatorTrait>>,
}

impl ClientBuilder {
    pub fn with_base_url(mut self, base_url: impl AsRef<str>) -> Result<Self> {
        let base_url = Url::parse(base_url.as_ref())?;
        self.base_url = Some(base_url);
        Ok(self)
    }

    pub fn with_version(mut self, version: impl AsRef<str>) -> Result<Self> {
        let base_url = self
            .base_url
            .as_mut()
            .ok_or(Error::ClientBuilderMissBaseUrl)?
            .join(version.as_ref())?;
        self.base_url = Some(base_url);
        Ok(self)
    }

    pub fn with_authenticator(
        mut self,
        authenticator: impl AuthenticatorTrait + 'static,
    ) -> Result<Self> {
        self.authenticator = Some(Box::new(authenticator));
        Ok(self)
    }

    pub fn build(self) -> Result<Client> {
        let Self {
            base_url,
            authenticator,
        } = self;

        let base_url = base_url.ok_or(Error::ClientBuilderMissBaseUrl)?;

        let authenticator = authenticator.ok_or(Error::ClientBuilderMissAuthenticator)?;

        Ok(Client {
            base_url,
            authenticator,
            client: reqwest::Client::new(),
        })
    }
}

pub struct Client {
    base_url: Url,
    authenticator: Box<dyn AuthenticatorTrait>,
    client: reqwest::Client,
}

impl Client {
    pub fn from_env_file(env: impl AsRef<str>) -> Result<Self> {
        let _ = dotenv::from_filename(env.as_ref());
        Self::from_env()
    }

    pub fn from_default_env() -> Result<Self> {
        let _ = dotenv::dotenv();
        Self::from_env()
    }

    pub fn from_env() -> Result<Self> {
        let base_url = std::env::var("OPENAI_API_BASE_URL")?;
        let key = std::env::var("OPENAI_API_KEY")?;
        let version = std::env::var("OPENAI_API_VERSION")?;
        Self::builder()
            .with_base_url(base_url)?
            .with_version(version)?
            .with_authenticator(Bearer::new(key))?
            .build()
    }

    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    pub async fn models(&self, timeout: Option<Duration>) -> Result<ModelListResponse> {
        let rep = self
            .call_impl(Method::GET, "models", [], None, None, timeout)
            .await?;

        let rep: ModelListResponse = serde_json::from_slice(rep.bytes().await?.as_ref())?;

        trace!("MODELS: {}", serde_json::to_string_pretty(&rep)?);

        Ok(rep)
    }

    pub async fn call_impl(
        &self,
        method: Method,
        uri: impl AsRef<str>,
        headers: impl IntoIterator<Item = (HeaderName, HeaderValue)>,
        body: Option<Body>,
        form: Option<Form>,
        timeout: Option<Duration>,
    ) -> Result<Response> {
        let path = std::path::PathBuf::from(self.base_url.path()).join(uri.as_ref());

        let url = self.base_url.join(path.to_str().expect("?"))?;

        let mut builder = self.client.request(method, url);

        if let Some(timeout) = timeout {
            builder = builder.timeout(timeout);
        }

        for (k, v) in headers.into_iter() {
            builder = builder.header(k, v);
        }

        if let Some(body) = body {
            builder = builder.body(body);
        }

        if let Some(form) = form {
            builder = builder.multipart(form);
        }

        let mut req = builder.build()?;

        self.authenticator.authorize(&mut req).await?;

        let rep = self.client.execute(req).await?; //.error_for_status()?;

        Ok(rep)
    }

    pub async fn generation(
        &self,
        req: GenerationRequest,
        timeout: Option<Duration>,
    ) -> Result<GenerationResponse> {
        req.call(&self, timeout).await
    }

    pub async fn chat_completion_stream(
        &self,
        mut req: ChatCompletionRequest,
        timeout: Option<Duration>,
    ) -> Result<Receiver<Result<ChatCompletionStreamData>>> {
        req.stream = Some(true);
        req.call_stream(&self, timeout).await
    }

    pub async fn chat_completion(
        &self,
        mut req: ChatCompletionRequest,
        timeout: Option<Duration>,
    ) -> Result<ChatCompletionResponse> {
        req.stream = Some(false);
        req.call_once(&self, timeout).await
    }
}
