use async_trait::async_trait;
use reqwest::Request;
use crate::error::*;
use http::header::{self, HeaderName, HeaderValue};
use tracing::*;

#[async_trait]
pub trait AuthenticatorTrait
{
    async fn authorize(&self, req: &mut Request) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct Bearer
{
    key: String,
}

impl Bearer
{
    pub fn new(key: String) -> Self
    {
        Self { key }
    }
}

#[async_trait]
impl AuthenticatorTrait for Bearer
{
    async fn authorize(&self, req: &mut Request) -> Result<()>
    {
        let k = header::AUTHORIZATION;
        let v = HeaderValue::from_str(&format!("Bearer {}", self.key))?;
        if let Some(k) = req.headers_mut().insert(k, v) {
            warn!("auth header {:?} exists and overwroted", k);
        }
        Ok(())
    }
}
