use crate::auth::*;
use crate::error::*;
use crate::proto::*;
use chat::ChatCompletionRequest;
use chat::ChatCompletionResponse;
use chat::ChatCompletionStreamData;
use futures::StreamExt;
use http::header;
use http::HeaderName;
use http::HeaderValue;
use http::Method;
use image::GenerationRequest;
use image::GenerationResponse;
use reqwest::Body;
use reqwest::Response;
use serde_json::Value;
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
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    pub async fn models(&self, timeout: Option<Duration>) -> Result<ModelListResponse> {
        let rep = self
            .call_impl(Method::GET, "models", [], None, timeout)
            .await?;

        let rep: ModelListResponse = serde_json::from_slice(rep.bytes().await?.as_ref())?;

        trace!("MODELS: {}", serde_json::to_string_pretty(&rep)?);

        // for l in serde_json::to_string_pretty(&rep)?.split("\n") {
        //     tracing::trace!("MODELS: {}", l);
        // }

        Ok(rep)
    }

    async fn call_impl(
        &self,
        method: Method,
        uri: impl AsRef<str>,
        headers: impl IntoIterator<Item = (HeaderName, HeaderValue)>,
        body: Option<Body>,
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

        let mut req = builder.build()?;

        self.authenticator.authorize(&mut req).await?;

        let rep = self.client.execute(req).await?.error_for_status()?;

        Ok(rep)
    }

    pub async fn generation(
        &self,
        req: GenerationRequest,
        timeout: Option<Duration>,
    ) -> Result<GenerationResponse> {
        let uri = "images/generations";

        let rep = self
            .call_impl(
                Method::POST,
                uri,
                vec![(
                    header::CONTENT_TYPE,
                    HeaderValue::from_str("application/json")?,
                )],
                Some(Body::from(serde_json::to_string(&req)?)),
                timeout,
            )
            .await?;

        let rep = serde_json::from_slice(rep.bytes().await?.as_ref())?;

        Ok(rep)
    }

    pub async fn chat_completion_stream(
        &self,
        mut req: ChatCompletionRequest,
        timeout: Option<Duration>,
    ) -> Result<Receiver<Result<ChatCompletionStreamData>>> {
        let uri = "chat/completions";

        req.stream = Some(true);

        let rep = self
            .call_impl(
                Method::POST,
                uri,
                vec![(
                    header::CONTENT_TYPE,
                    HeaderValue::from_str("application/json")?,
                )],
                Some(Body::from(serde_json::to_vec(&req)?)),
                timeout,
            )
            .await?;

        let (tx, rx) = tokio::sync::mpsc::channel(1);

        tokio::spawn(async move {
            let mut stack = vec![];
            let mut stream = rep.bytes_stream();

            let s_tag = "data: ".as_bytes();
            let s_tag_len = s_tag.len();
            let e_tag = "\n\n".as_bytes();
            let e_tag_len = e_tag.len();

            while let Some(r) = stream.next().await {
                let chunk = match r {
                    Ok(r) => r,
                    Err(e) => {
                        error!("stream return with error: {:?}", e);
                        break;
                    }
                };

                debug!("recv {} bytes", chunk.len());

                for b in chunk.as_ref() {
                    stack.push(*b);
                    if stack.len() >= e_tag_len + s_tag_len {
                        let slice = &stack[stack.len() - e_tag_len..];

                        if slice == e_tag {
                            let mut data = vec![];
                            std::mem::swap(&mut data, &mut stack);

                            let data =
                                String::from_utf8_lossy(&data[s_tag_len..data.len() - e_tag_len]);

                            if data.find("[DONE]").is_some() {
                                trace!("met [DONE], data={}", data);
                                continue;
                            }

                            match serde_json::from_str::<ChatCompletionStreamData>(&data) {
                                Err(e) => {
                                    error!("failed to parse data: error={:?}, data={}", e, data);
                                    tx.send(Err(e.into())).await.map_err(|_| {
                                        error!("failed to send error message to chat receiver");
                                        Error::SendMessage
                                    })?;
                                }
                                Ok(data) => {
                                    tx.send(Ok(data)).await.map_err(|_| {
                                        error!("failed to send data message to chat receiver");
                                        Error::SendMessage
                                    })?;
                                }
                            }
                        }
                    }
                }
            }
            trace!("stream thread quit, with stack.len()={}", stack.len());
            Result::Ok(())
        });

        Ok(rx)
    }

    pub async fn chat_completion(
        &self,
        mut req: ChatCompletionRequest,
        timeout: Option<Duration>,
    ) -> Result<ChatCompletionResponse> {
        let uri = "chat/completions";

        req.stream = Some(false);

        let rep = self
            .call_impl(
                Method::POST,
                uri,
                vec![(
                    header::CONTENT_TYPE,
                    HeaderValue::from_str("application/json")?,
                )],
                Some(Body::from(serde_json::to_vec(&req)?)),
                timeout,
            )
            .await?;

        let status = rep.status();

        let rep = serde_json::from_slice::<Value>(rep.bytes().await?.as_ref())?;

        for l in serde_json::to_string_pretty(&rep)?.split("\n") {
            if status.is_success() {
                tracing::trace!("RESPONSE: {}", l);
            } else {
                tracing::error!("RESPONSE: {}", l);
            }
        }

        if status.is_success() {
            let rep: ChatCompletionResponse = serde_json::from_value(rep)?;
            Ok(rep)
        } else {
            error!("chat completion failed");
            Err(Error::ApiError(status.as_u16()))
        }
    }
}

#[cfg(test)]
#[tokio::test]
async fn test_generation_ok() -> anyhow::Result<()> {
    use chat::{ImageUrl, Message};
    use image::{GenerationData, GenerationFormat, GenerationRequest};

    let _ = dotenv::from_filename(".env.stepfun.genai");
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

    let req = GenerationRequest::builder()
        .with_model(model_name)
        .with_prompt("教室里快乐的小孩在做实验")
        .with_size(1280, 800)
        .build()?;

    let rep = client.generation(req, None).await?;

    let GenerationResponse { created, data } = rep;

    let data = data.into_iter().next().expect("no data");

    info!(?data);

    let GenerationData {
        seed,
        finish_reason,
        image,
        url,
    } = data;

    if let Some(image) = image {
        info!("image data={}, len={}", &image[0..20], image.len())
    }

    if let Some(url) = url {
        info!(%url);
        let req = ChatCompletionRequest::builder()
            .with_model("step-1v-8k")
            .add_message(
                Message::builder()
                    .with_role(chat::Role::system)
                    .with_content("你是一个大模型")
                    .build(),
            )
            .add_message(
                Message::builder()
                    .with_role(chat::Role::user)
                    .add_content("分析下这张图片")
                    .add_content(ImageUrl::from_url(url))
                    .build(),
            )
            .build()?;

        let rep = client.chat_completion(req, None).await?;

        for l in serde_json::to_string_pretty(&rep)?.split("\n") {
            info!("RESPONSE: {}", l);
        }
    }

    Ok(())
}

#[cfg(test)]
#[tokio::test]
async fn test_chat_ok() -> anyhow::Result<()> {
    use std::collections::HashMap;

    use chat::{Argument, Function, Message, ParameterProperty, ParameterType, Parameters, Role};

    let _ = dotenv::from_filename(".env.stepfun");
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

    let messages = vec![
        Message::builder()
            .with_role(Role::system)
            .with_content("你是一个大模型")
            .build(),
        Message::builder()
            .with_role(Role::user)
            .with_content("计算 1921.23 + 42.00")
            .build(),
    ];

    let add_number_function = Function::builder()
        .with_name("add_number")
        .with_description("将两个数字相加")
        .with_parameters(
            Parameters::builder()
                .add_property(
                    "a",
                    ParameterProperty::builder()
                        .with_description("加数")
                        .with_type(ParameterType::number)
                        .build()?,
                )
                .add_required("a")
                .add_property(
                    "b",
                    ParameterProperty::builder()
                        .with_description("加数")
                        .with_type(ParameterType::number)
                        .build()?,
                )
                .add_required("b")
                .build()?,
        )
        .build()?;

    let req = ChatCompletionRequest::builder()
        .with_model(model_name)
        .with_messages(messages)
        .with_tool(add_number_function)
        .build()?;

    for l in serde_json::to_string_pretty(&req)?.split("\n") {
        info!("REQUEST: {}", l);
    }

    let rep = match use_stream {
        true => {
            let mut rx = client.chat_completion_stream(req, None).await?;
            let mut rep = ChatCompletionResponse::default();
            while let Some(r) = rx.recv().await {
                let data = match r {
                    Ok(r) => r,
                    Err(e) => {
                        error!("failed to process stream delta: {:?}", e);
                        continue;
                    }
                };

                for l in serde_json::to_string_pretty(&data)?.split("\n") {
                    trace!("STREAM DATA: {}", l);
                }

                rep.merge_delta(data);
            }
            for l in serde_json::to_string_pretty(&rep)?.split("\n") {
                info!("RESPONSE: {}", l);
            }
            rep
        }
        false => {
            let rep = client.chat_completion(req, None).await?;
            for l in serde_json::to_string_pretty(&rep)?.split("\n") {
                info!("RESPONSE: {}", l);
            }
            rep
        }
    };

    let arguments = rep
        .choices
        .last()
        .and_then(|c| c.message.tool_calls.first())
        .and_then(|t| t.function.arguments.as_ref())
        .and_then(|args| serde_json::from_str::<HashMap<String, Argument>>(args).ok())
        .expect("function call failed");

    for l in serde_json::to_string_pretty(&arguments)?.lines() {
        info!("ARGUMENTS: {}", l);
    }

    let a = arguments.get("a").and_then(|a| a.as_number()).unwrap();
    let b = arguments.get("b").and_then(|a| a.as_number()).unwrap();

    info!("add {}+{}={}", a, b, a + b);

    Ok(())
}
