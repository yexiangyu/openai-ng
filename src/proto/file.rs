use http::Method;
use reqwest::{
    multipart::{Form, Part},
    Body,
};
use serde_json::Value;
use smart_default::SmartDefault;
use std::{path::PathBuf, time::Duration};
use tokio_util::codec::{BytesCodec, FramedRead};
use tracing::*;
use url::Url;

use crate::{client::Client, error::*};

pub struct FileContentRequest {
    pub id: String,
}

impl FileContentRequest {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    pub async fn call(
        &self,
        client: &Client,
        timeout: Option<Duration>,
    ) -> Result<FileContentResponse> {
        let rep = client
            .call_impl(
                Method::GET,
                &format!("files/{}/content", self.id),
                vec![],
                None,
                None,
                timeout,
            )
            .await?;

        let status = rep.status();

        let rep: Value = serde_json::from_slice(rep.bytes().await?.as_ref())?;

        for l in serde_json::to_string_pretty(&rep)?.lines() {
            if status.is_success() {
                trace!(%l, "REP");
            } else {
                error!(%l, "REP");
            }
        }

        if !status.is_success() {
            return Err(Error::ApiError(status.as_u16()));
        }

        let rep: FileContentResponse = serde_json::from_value(rep)?;

        for l in rep.content.lines() {
            trace!(%l, "REP");
        }

        Ok(rep)
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct FileContentResponse {
    pub file_type: String,
    pub filename: String,
    pub title: String,
    #[serde(rename = "type")]
    pub typ: String,
    pub content: String,
}

pub struct FileDeleteRequest {
    pub id: String,
}

impl FileDeleteRequest {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    pub async fn call(&self, client: &Client, timeout: Option<Duration>) -> Result<()> {
        let rep = client
            .call_impl(
                Method::DELETE,
                &format!("files/{}", self.id),
                vec![],
                None,
                None,
                timeout,
            )
            .await?;

        let status = rep.status();

        let rep: Value = serde_json::from_slice(rep.bytes().await?.as_ref())?;

        for l in serde_json::to_string_pretty(&rep)?.lines() {
            if status.is_success() {
                trace!(%l, "REP");
            } else {
                error!(%l, "REP");
            }
        }

        if !status.is_success() {
            return Err(Error::ApiError(status.as_u16()));
        }

        Ok(())
    }
}

pub struct FileGetRequest {
    pub id: String,
}

impl FileGetRequest {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    pub async fn call(
        &self,
        client: &Client,
        timeout: Option<Duration>,
    ) -> Result<FileUploadResponse> {
        let rep = client
            .call_impl(
                Method::GET,
                &format!("files/{}", self.id),
                vec![],
                None,
                None,
                timeout,
            )
            .await?;

        let status = rep.status();

        let rep: Value = serde_json::from_slice(rep.bytes().await?.as_ref())?;

        for l in serde_json::to_string_pretty(&rep)?.lines() {
            if status.is_success() {
                trace!(%l, "REP");
            } else {
                error!(%l, "REP");
                return Err(Error::ApiError(status.as_u16()));
            }
        }

        Ok(serde_json::from_value(rep)?)
    }
}

pub struct FileListRequest;

impl FileListRequest {
    pub async fn call(
        &self,
        client: &Client,
        timeout: Option<Duration>,
    ) -> Result<FileListResponse> {
        let rep = client
            .call_impl(Method::GET, "files", vec![], None, None, timeout)
            .await?;

        let status = rep.status();

        let rep: Value = serde_json::from_slice(rep.bytes().await?.as_ref())?;

        for l in serde_json::to_string_pretty(&rep)?.lines() {
            if status.is_success() {
                trace!(%l, "REP");
            } else {
                error!(%l, "REP");
                return Err(Error::ApiError(status.as_u16()));
            }
        }

        Ok(serde_json::from_value(rep)?)
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FileListResponse {
    pub object: String,
    pub data: Vec<FileUploadResponse>,
}

impl From<PathBuf> for FileSource {
    fn from(value: PathBuf) -> Self {
        Self::Local(value)
    }
}

impl From<Url> for FileSource {
    fn from(value: Url) -> Self {
        Self::Remote {
            url: value,
            trust_all_certification: true,
        }
    }
}

pub enum FileSource {
    Local(PathBuf),
    Remote {
        url: Url,
        trust_all_certification: bool,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, SmartDefault)]
pub enum FilePurpose {
    #[default]
    #[serde(rename = "file-extract")]
    Extract,
}

impl From<FilePurpose> for String {
    fn from(value: FilePurpose) -> Self {
        match value {
            FilePurpose::Extract => "file-extract".to_string(),
        }
    }
}

impl From<&FilePurpose> for String {
    fn from(value: &FilePurpose) -> Self {
        match value {
            FilePurpose::Extract => "file-extract".to_string(),
        }
    }
}

pub struct FileUploadRequest {
    pub source: FileSource,
    pub purpose: FilePurpose,
}

impl FileUploadRequest {
    pub async fn call(
        &self,
        client: &Client,
        timeout: Option<Duration>,
    ) -> Result<FileUploadResponse> {
        let part = match &self.source {
            FileSource::Local(local_path) => {
                let file_name = local_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .ok_or(Error::NoFileName)?;
                let file = tokio::fs::File::open(local_path).await?;
                let stream = FramedRead::new(file, BytesCodec::new());
                let file_body = Body::wrap_stream(stream);
                let some_file = Part::stream(file_body).file_name(file_name);
                some_file
            }
            FileSource::Remote {
                url,
                trust_all_certification,
            } => {
                let filename = PathBuf::from(url.path())
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .ok_or(Error::NoFileName)?;

                trace!(%trust_all_certification, %filename, "upload remote url={}", url.as_str());

                let rep = reqwest::Client::builder()
                    .danger_accept_invalid_certs(*trust_all_certification)
                    .build()?
                    .get(url.clone())
                    .send()
                    .await?;

                let bytes = rep.bytes().await?;

                let some_file = Part::stream(bytes).file_name(filename);

                some_file
            }
        };

        let purpose = String::from(&self.purpose);

        info!(?purpose);

        let form = Form::new()
            .text("purpose", String::from(&self.purpose))
            .part("file", part);

        let rep = client
            .call_impl(Method::POST, "files", vec![], None, Some(form), timeout)
            .await?;

        let status = rep.status();

        let rep: Value = serde_json::from_slice(rep.bytes().await?.as_ref())?;

        for l in serde_json::to_string_pretty(&rep)?.lines() {
            if status.is_success() {
                trace!(%l, "REP");
            } else {
                error!(%l, "REP");
                return Err(Error::ApiError(status.as_u16()));
            }
        }

        Ok(serde_json::from_value(rep)?)
    }
}

impl FileUploadRequest {
    pub fn builder() -> FileUploadRequestBuilder {
        FileUploadRequestBuilder::default()
    }
}

#[derive(SmartDefault)]
pub struct FileUploadRequestBuilder {
    source: Option<FileSource>,
    purpose: FilePurpose,
}

impl FileUploadRequestBuilder {
    pub fn with_source(mut self, source: impl Into<FileSource>) -> Self {
        self.source = Some(source.into());
        self
    }

    pub fn with_purpose(mut self, purpose: FilePurpose) -> Self {
        self.purpose = purpose;
        self
    }

    pub fn build(self) -> Result<FileUploadRequest> {
        Ok(FileUploadRequest {
            source: self.source.ok_or(Error::FileRequestBuild)?,
            purpose: self.purpose,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileUploadResponse {
    pub id: String,
    pub object: String,
    pub bytes: usize,
    pub created_at: u64,
    pub filename: String,
    pub purpose: FilePurpose,
    pub status: String,
    pub status_details: String,
}

#[cfg(test)]
#[tokio::test]
async fn test_file_upload_ok() -> anyhow::Result<()> {
    use crate::auth::Bearer;
    let _ = dotenv::from_filename(".env.kimi");

    let _ = tracing_subscriber::fmt::try_init();
    let base_url = std::env::var("OPENAI_API_BASE_URL")?;
    let key = std::env::var("OPENAI_API_KEY")?;
    let version = std::env::var("OPENAI_API_VERSION")?;
    let model_name = std::env::var("OPENAI_API_MODEL_NAME")?;
    let vision_available = std::env::var("OPENAI_API_VISION").is_ok();
    let use_stream = std::env::var("USE_STREAM").is_ok();

    info!(%base_url, %key, %version, %model_name, %vision_available, %use_stream, "start test with");

    let client = Client::builder()
        .with_authenticator(Bearer::new(key))?
        .with_base_url(base_url)?
        .with_version(version)?
        .build()?;

    let rep = FileListRequest.call(&client, None).await?;

    for item in &rep.data {
        if item.filename != "161528_24 司马光 优质教案.pdf" {
            continue;
        }
        let rep = FileGetRequest::new(&item.id).call(&client, None).await?;
        let rep = FileContentRequest::new(&item.id)
            .call(&client, None)
            .await?;
    }

    // let source =
    //     PathBuf::from("/Users/yexiangyu/Repo/openai-ng/tests/161528_24 司马光 优质教案.pdf");

    // // let source = Url::parse("https://changkun.de/modern-cpp/pdf/modern-cpp-tutorial-en-us.pdf")?;

    // let req = FileUploadRequest::builder()
    //     .with_source(source)
    //     .with_purpose(FilePurpose::Extract)
    //     .build()?;

    // let rep = req.call(&client, None).await?;

    Ok(())
}
