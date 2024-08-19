use crate::auth::*;
use crate::error::*;
use crate::proto::*;
use http::HeaderName;
use http::HeaderValue;
use http::Method;
use reqwest::multipart::Form;
use reqwest::Body;
use reqwest::Response;
use smart_default::SmartDefault;
use std::time::Duration;
use sys::ModelListResponse;
use tracing::*;
use url::Url;

/// Client builder
/// ```rust
/// use openai_ng::prelude::*;
///
/// let builder = Client::builder();
/// let client = builder
///                 .with_base_url("https://api.openai.com")?
///                 .with_version("v1")?
///                 .with_key("you client key")?
///                 .build()?;
/// ```
#[derive(SmartDefault)]
pub struct ClientBuilder {
    pub base_url: Option<Url>,
    pub authenticator: Option<Box<dyn AuthenticatorTrait>>,
}

impl ClientBuilder {
    /// config base_url
    pub fn with_base_url(mut self, base_url: impl AsRef<str>) -> Result<Self> {
        let base_url = Url::parse(base_url.as_ref())?;
        self.base_url = Some(base_url);
        Ok(self)
    }

    /// config version
    pub fn with_version(mut self, version: impl AsRef<str>) -> Result<Self> {
        let base_url = self
            .base_url
            .as_mut()
            .ok_or(Error::ClientBuild)?
            .join(version.as_ref())?;
        self.base_url = Some(base_url);
        Ok(self)
    }

    /// config bearer authenticator with key
    pub fn with_key(self, key: impl AsRef<str>) -> Result<Self> {
        self.with_authenticator(Bearer::new(key.as_ref().to_string()))
    }

    /// config authenticator with custom authenticator
    pub fn with_authenticator(
        mut self,
        authenticator: impl AuthenticatorTrait + 'static,
    ) -> Result<Self> {
        self.authenticator = Some(Box::new(authenticator));
        Ok(self)
    }

    /// build client
    pub fn build(self) -> Result<Client> {
        let Self {
            base_url,
            authenticator,
        } = self;

        let base_url = base_url.ok_or(Error::ClientBuild)?;

        let authenticator = authenticator.ok_or(Error::ClientBuild)?;

        Ok(Client {
            base_url,
            authenticator,
            client: reqwest::Client::new(),
        })
    }
}

/// OpenAI API client
pub struct Client {
    base_url: Url,
    authenticator: Box<dyn AuthenticatorTrait>,
    client: reqwest::Client,
}

impl Client {
    /// create client from customized env file, convenient for development, use `dotenv` crate
    pub fn from_env_file(env: impl AsRef<str>) -> Result<Self> {
        let _ = dotenv::from_filename(env.as_ref());
        Self::from_env()
    }

    /// create client from default env file: `.env`, convenient for development, use `dotenv` crate
    pub fn from_default_env() -> Result<Self> {
        let _ = dotenv::dotenv();
        Self::from_env()
    }

    /// create client from environment variables
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

    /// create a client builder
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// list all models available
    pub async fn models(&self, timeout: Option<Duration>) -> Result<ModelListResponse> {
        let rep = self
            .call_impl(Method::GET, "models", [], None, None, timeout)
            .await?;

        let status = rep.status();

        let rep: serde_json::Value = serde_json::from_slice(rep.bytes().await?.as_ref())?;

        for l in serde_json::to_string_pretty(&rep)?.lines() {
            if status.is_client_error() || status.is_server_error() {
                error!("REP: {}", l);
            } else {
                trace!("REP: {}", l);
            }
        }

        if !status.is_success() {
            return Err(Error::ApiError(status.as_u16()));
        }

        Ok(serde_json::from_value(rep)?)
    }

    /// do the actual call
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
}
