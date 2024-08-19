use crate::error::*;
use async_trait::async_trait;
use http::header::{self, HeaderValue};
use reqwest::Request;
use tracing::*;

/// trait to authorize `reqwest::Request`, might add more authorization method in the future
#[async_trait]
pub trait AuthenticatorTrait {
    async fn authorize(&self, req: &mut Request) -> Result<()>;
}

/// Bearer token authorization
#[derive(Debug, Clone)]
pub struct Bearer {
    key: String,
}

impl Bearer {
    /// create a new Bearer token authorization
    pub fn new(key: String) -> Self {
        Self { key }
    }
}

#[async_trait]
impl AuthenticatorTrait for Bearer {
    async fn authorize(&self, req: &mut Request) -> Result<()> {
        let k = header::AUTHORIZATION;
        let v = HeaderValue::from_str(&format!("Bearer {}", self.key))?;
        if let Some(k) = req.headers_mut().insert(k, v) {
            warn!("auth header {:?} exists and overwroted", k);
        }
        Ok(())
    }
}
