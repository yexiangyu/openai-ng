use crate::client::Client;
use crate::error::*;
use crate::proto::tool::*;

use base64::Engine;
use futures::StreamExt;
use http::{
    header::{self, HeaderValue},
    Method,
};
use reqwest::Body;
use serde::de::{Deserialize, IntoDeserializer};
use serde_with::skip_serializing_none;
use smart_default::SmartDefault;
use tokio::sync::mpsc::Receiver;
use tracing::*;

use std::time::Duration;

#[skip_serializing_none]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, SmartDefault)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub tools: Vec<ToolCall>,
    pub max_tokens: Option<u64>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub n: Option<u64>,
    pub stream: Option<bool>,
    pub stop: Option<Stop>,
    pub frequency_penalty: Option<f64>,
    pub response_format: Option<ResponseFormat>,
}

pub enum ChatCompletionResult {
    Response(ChatCompletionResponse),
    Delta(Receiver<Result<ChatCompletionStreamData>>),
}

impl ChatCompletionRequest {
    pub async fn call_once(
        &self,
        client: &Client,
        timeout: Option<Duration>,
    ) -> Result<ChatCompletionResponse> {
        let uri = "chat/completions";

        let rep = client
            .call_impl(
                Method::POST,
                uri,
                vec![(
                    header::CONTENT_TYPE,
                    HeaderValue::from_str("application/json")?,
                )],
                Some(Body::from(serde_json::to_vec(&self)?)),
                None,
                timeout,
            )
            .await?;

        let status = rep.status();

        let rep = serde_json::from_slice::<serde_json::Value>(rep.bytes().await?.as_ref())?;

        for l in serde_json::to_string_pretty(&rep)?.split("\n") {
            if status.is_success() {
                tracing::trace!("REP: {}", l);
            } else {
                tracing::error!("REP: {}", l);
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

    pub async fn call_stream(
        &self,
        client: &Client,
        timeout: Option<Duration>,
    ) -> Result<Receiver<Result<ChatCompletionStreamData>>> {
        let uri = "chat/completions";

        let rep = client
            .call_impl(
                Method::POST,
                uri,
                vec![(
                    header::CONTENT_TYPE,
                    HeaderValue::from_str("application/json")?,
                )],
                Some(Body::from(serde_json::to_vec(&self)?)),
                None,
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

                trace!("recv chunk {} bytes", chunk.len());

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
                                    trace!("found data event from stream");
                                    for l in serde_json::to_string_pretty(&data)?.lines() {
                                        trace!("DATA: {}", l);
                                    }
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

    pub async fn call(
        &self,
        client: &crate::client::Client,
        timeout: Option<std::time::Duration>,
    ) -> Result<ChatCompletionResult> {
        match self.stream {
            Some(true) => Ok(ChatCompletionResult::Delta(
                self.call_stream(client, timeout).await?,
            )),
            _ => Ok(ChatCompletionResult::Response(
                self.call_once(client, timeout).await?,
            )),
        }
    }
}

#[skip_serializing_none]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    typ: ResponseType,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(non_camel_case_types)]
pub enum ResponseType {
    json_object,
}

#[derive(Debug, Clone, SmartDefault)]
pub struct ChatCompletionRequestBuilder {
    model: Option<String>,
    messages: Vec<Message>,
    tools: Vec<ToolCall>,
    max_tokens: Option<u64>,
    temperature: Option<f64>,
    top_p: Option<f64>,
    n: Option<u64>,
    stream: Option<bool>,
    stop: Option<Stop>,
    frequency_penalty: Option<f64>,
    response_format: Option<ResponseFormat>,
}

impl ChatCompletionRequestBuilder {
    pub fn with_reponse_format(mut self, format: ResponseType) -> Self {
        self.response_format = Some(ResponseFormat { typ: format });
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn with_messages(mut self, messages: impl IntoIterator<Item = Message>) -> Self {
        self.messages.extend(messages);
        self
    }

    pub fn add_message(mut self, msg: Message) -> Self {
        self.messages.push(msg);
        self
    }

    pub fn with_tool(mut self, tool: impl Into<ToolCall>) -> Self {
        self.tools.push(tool.into());
        self
    }

    pub fn with_tools<T>(mut self, tools: impl IntoIterator<Item = T>) -> Self
    where
        T: Into<ToolCall>,
    {
        self.tools.extend(tools.into_iter().map(|t| t.into()));
        self
    }

    pub fn add_tool(self, tool: impl Into<ToolCall>) -> Self {
        self.with_tool(tool)
    }

    pub fn with_max_tokens(mut self, max_tokens: u64) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn with_temperature(mut self, temperature: f64) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn with_n(mut self, n: u64) -> Self {
        self.n = Some(n);
        self
    }

    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = Some(stream);
        self
    }

    pub fn with_stop(mut self, rhs: Stop) -> Self {
        self.stop = Some(rhs);
        self
    }

    pub fn add_stop(mut self, rhs: Stop) -> Self {
        let mut lhs = None;
        std::mem::swap(&mut self.stop, &mut lhs);
        self.stop = match lhs {
            None => Some(rhs),
            Some(lhs) => Some(lhs.append(rhs)),
        };
        self
    }

    pub fn with_frequency_penalty(mut self, frequency_penalty: f64) -> Self {
        self.frequency_penalty = Some(frequency_penalty);
        self
    }

    pub fn build(self) -> Result<ChatCompletionRequest> {
        let Self {
            model,
            messages,
            tools,
            max_tokens,
            temperature,
            top_p,
            n,
            stream,
            stop,
            frequency_penalty,
            response_format,
        } = self;

        let model = model.ok_or(Error::ChatCompletionRequestBuild)?;

        if messages.is_empty() {
            return Err(Error::ChatCompletionRequestBuild);
        }

        let r = ChatCompletionRequest {
            model,
            messages,
            tools,
            max_tokens,
            temperature,
            top_p,
            n,
            stream,
            stop,
            frequency_penalty,
            response_format,
        };

        for l in serde_json::to_string_pretty(&r)?.lines() {
            trace!("REQ: {}", l);
        }

        Ok(r)
    }
}

impl ChatCompletionRequest {
    pub fn builder() -> ChatCompletionRequestBuilder {
        ChatCompletionRequestBuilder::default()
    }
}

#[skip_serializing_none]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, SmartDefault)]
pub struct ChatCompletionResponse {
    pub id: String,
    #[default("chat.completion".to_string())]
    pub object: String,
    pub created: u64,
    pub model: String,
    #[serde(default)]
    pub choices: Vec<Choice>,
    pub usage: Option<ChatComplitionUsage>,
}

impl ChatCompletionResponse {
    pub fn merge_delta(&mut self, delta: ChatCompletionStreamData) {
        let ChatCompletionStreamData {
            id,
            object,
            created,
            model,
            choices,
            usage,
        } = delta;

        if let Some(usage) = usage {
            self.usage = Some(usage);
        }

        if let Some(id) = id {
            self.id = id;
        }

        if let Some(object) = object {
            if self.object.is_empty() {
                self.object = object;
            }
        }

        if let Some(created) = created {
            self.created = created;
        }

        if let Some(model) = model {
            self.model = model;
        }

        'outer: for delta in choices {
            let StreamChoice {
                index,
                delta,
                finish_reason,
                usage,
            } = delta;

            if let Some(usage) = usage {
                self.usage = Some(usage);
            }

            let Message {
                role,
                content,
                tool_calls,
                tool_call_id,
            } = delta;

            for choice in &mut self.choices {
                if choice.index == index {
                    if let Some(role) = role {
                        choice.message.role = Some(role);
                    }

                    if let Some(delta_content) = content {
                        let mut choice_content = None;
                        std::mem::swap(&mut choice.message.content, &mut choice_content);
                        match choice_content.as_mut() {
                            Some(c) => c.merge(delta_content),
                            None => choice_content = Some(delta_content),
                        };
                        std::mem::swap(&mut choice.message.content, &mut choice_content);
                    }

                    if let Some(tool_call_id) = tool_call_id {
                        choice.message.tool_call_id = Some(tool_call_id);
                    }

                    if choice.message.tool_calls.is_empty() {
                        choice.message.tool_calls = tool_calls;
                    } else {
                        choice
                            .message
                            .tool_calls
                            .iter_mut()
                            .zip(tool_calls)
                            .for_each(|(lhs, rhs)| {
                                if let Some(name) = rhs.function.name.as_ref() {
                                    if !name.is_empty() {
                                        lhs.function.name = Some(name.clone());
                                    }
                                }

                                match (&mut lhs.function.arguments, &rhs.function.arguments) {
                                    (Some(lhs), Some(rhs)) => {
                                        *lhs = format!("{}{}", lhs, rhs);
                                    }
                                    (None, Some(rhs)) => {
                                        lhs.function.arguments = Some(rhs.clone());
                                    }
                                    _ => {}
                                }
                            });
                    }

                    if let Some(finish_reason) = finish_reason {
                        choice.finish_reason = Some(finish_reason);
                    }

                    continue 'outer;
                }
            }

            self.choices.push(Choice {
                index,
                message: Message {
                    role,
                    content,
                    tool_call_id,
                    tool_calls,
                },
                finish_reason,
            });
        }
    }
}

#[skip_serializing_none]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Choice {
    pub index: usize,
    pub message: Message,
    pub finish_reason: Option<String>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Stop {
    Text(String),
    Texts(Vec<String>),
}

impl Stop {
    pub fn append(self, rhs: Stop) -> Self {
        match (self, rhs) {
            (Stop::Text(lhs), Stop::Text(rhs)) => Stop::Texts(vec![lhs, rhs]),
            (Stop::Text(lhs), Stop::Texts(mut rhs)) => {
                rhs.push(lhs);
                Stop::Texts(rhs)
            }
            (Stop::Texts(mut lhs), Stop::Text(rhs)) => {
                lhs.push(rhs);
                Stop::Texts(lhs)
            }
            (Stop::Texts(mut lhs), Stop::Texts(rhs)) => {
                lhs.extend(rhs);
                Stop::Texts(lhs)
            }
        }
    }
}

#[skip_serializing_none]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, SmartDefault)]
pub struct ChatComplitionUsage {
    pub cached_tokens: Option<u64>,
    pub completion_tokens: u64,
    pub prompt_tokens: u64,
    pub total_tokens: u64,
}

#[skip_serializing_none]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, SmartDefault)]
pub struct Message {
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub role: Option<Role>,
    pub content: Option<Content>,
    pub tool_call_id: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
}

fn empty_string_as_none<'de, D>(de: D) -> std::result::Result<Option<Role>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(de)?;
    match opt.as_deref() {
        None | Some("") => Ok(None),
        Some(s) => Role::deserialize(s.into_deserializer()).map(Some),
    }
}

impl Message {
    pub fn builder() -> MessageBuilder {
        MessageBuilder::default()
    }
}

#[derive(SmartDefault)]
pub struct MessageBuilder {
    role: Option<Role>,
    content: Option<Content>,
    tool_call_id: Option<String>,
    tool_calls: Vec<ToolCall>,
}

impl MessageBuilder {
    pub fn with_role(mut self, role: Role) -> Self {
        self.role = Some(role);
        self
    }

    pub fn with_content(mut self, content: impl Into<Content>) -> Self {
        self.content = Some(content.into());
        self
    }

    // pub fn add_content(mut self, content: impl Into<Content>) -> Self {
    //     let mut lhs = None;
    //     std::mem::swap(&mut self.content, &mut lhs);
    //     self.content = match lhs {
    //         None => Some(content.into()),
    //         Some(lhs) => Some(lhs.merge(content)),
    //     };
    //     self
    // }

    pub fn with_tool_call_id(mut self, tool_call_id: impl Into<String>) -> Self {
        self.tool_call_id = Some(tool_call_id.into());
        self
    }

    pub fn with_tool_calls(mut self, tool_calls: Vec<ToolCall>) -> Self {
        self.tool_calls = tool_calls;
        self
    }

    pub fn add_tool_call(mut self, tool_call: ToolCall) -> Self {
        self.tool_calls.push(tool_call);
        self
    }

    pub fn build(self) -> Message {
        let Self {
            role,
            content,
            tool_call_id,
            tool_calls,
        } = self;

        Message {
            role,
            content,
            tool_call_id,
            tool_calls,
        }
    }
}

#[skip_serializing_none]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(non_camel_case_types)]
pub enum Role {
    system,
    user,
    assistant,
    tool,
}

#[skip_serializing_none]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum Content {
    Text(String),
    Containers(Vec<ContentContainer>),
}

impl From<String> for Content {
    fn from(s: String) -> Self {
        Content::Text(s)
    }
}

impl From<&str> for Content {
    fn from(s: &str) -> Self {
        Content::Text(s.to_string())
    }
}

impl From<ImageUrl> for Content {
    fn from(url: ImageUrl) -> Self {
        Content::Containers(vec![ContentContainer::Image {
            typ: "image_url".into(),
            image_url: url,
        }])
    }
}

impl Content {
    pub fn from_image_url(url: &str) -> Self {
        Content::Containers(vec![ContentContainer::Image {
            typ: "image_url".into(),
            image_url: ImageUrl::from_url(url),
        }])
    }

    pub fn from_text(text: impl Into<String>) -> Self {
        Content::Text(text.into())
    }

    pub fn merge(&mut self, rhs: Self) {
        *self = match self {
            Content::Text(s0) => match rhs {
                Content::Text(s1) => {
                    *s0 += s1.as_str();
                    return;
                }
                Content::Containers(cs) => {
                    let mut cs_ = vec![s0.clone().into()];
                    cs_.extend(cs);
                    Content::Containers(cs_)
                }
            },
            Content::Containers(cs) => {
                match rhs {
                    Content::Text(s1) => cs.push(ContentContainer::Text {
                        typ: "text".into(),
                        text: s1,
                    }),
                    Content::Containers(cs_) => cs.extend(cs_),
                }
                return;
            }
        };
    }

    pub fn append(&mut self, item: impl Into<ContentContainer>) {
        *self = match self {
            Content::Text(s) => Content::Containers(vec![
                ContentContainer::Text {
                    typ: "text".into(),
                    text: s.clone(),
                },
                item.into(),
            ]),
            Content::Containers(cs) => {
                let mut cs_ = vec![];
                std::mem::swap(cs, &mut cs_);
                cs_.push(item.into());
                Content::Containers(cs_)
            }
        };
    }
}

#[skip_serializing_none]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum ContentContainer {
    Text {
        #[serde(rename = "type")]
        typ: String,
        text: String,
    },
    Image {
        #[serde(rename = "type")]
        typ: String,
        image_url: ImageUrl,
    },
}

#[skip_serializing_none]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImageUrl {
    pub url: String,
}

impl From<ImageUrl> for ContentContainer {
    fn from(url: ImageUrl) -> Self {
        ContentContainer::Image {
            typ: "image".into(),
            image_url: url,
        }
    }
}

impl From<String> for ContentContainer {
    fn from(s: String) -> Self {
        ContentContainer::Text {
            typ: "text".into(),
            text: s,
        }
    }
}

impl ImageUrl {
    pub async fn from_local_file(path: impl Into<std::path::PathBuf>) -> Result<Self> {
        let path = path.into();
        let suffix = path
            .extension()
            .ok_or(Error::NoFileExtension)?
            .to_str()
            .ok_or(Error::NoFileExtension)?;
        let binary = tokio::fs::read(&path).await?;
        Ok(ImageUrl::from_image_binary(binary, suffix))
    }

    pub fn from_url(url: impl Into<String>) -> Self {
        ImageUrl { url: url.into() }
    }

    pub fn from_image_binary(image: impl AsRef<[u8]>, suffix: impl AsRef<str>) -> Self {
        ImageUrl {
            url: format!(
                "data:image/{};base64,{}",
                suffix.as_ref(),
                base64::prelude::BASE64_STANDARD.encode(image)
            ),
        }
    }
}

#[skip_serializing_none]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatCompletionStreamData {
    pub id: Option<String>,
    pub object: Option<String>,
    pub created: Option<u64>,
    pub model: Option<String>,
    pub choices: Vec<StreamChoice>,
    pub usage: Option<ChatComplitionUsage>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StreamChoice {
    pub index: usize,
    pub delta: Message,
    pub finish_reason: Option<String>,
    pub usage: Option<ChatComplitionUsage>,
}

#[allow(dead_code)]
#[cfg(test)]
async fn example_code() -> Result<()> {
    // build a client
    let client = Client::builder()
        .with_base_url("https://api.stepfun.com")?
        .with_key("you api key")?
        .with_version("v1")?
        .build()?;

    // build a request
    let req = ChatCompletionRequest::builder()
        .with_model("step-1-8k")
        .with_messages([
            Message::builder()
                .with_role(Role::system)
                .with_content("you are a good llm model")
                .build(),
            Message::builder()
                .with_role(Role::user)
                .with_content("calculate 1921.23 + 42.00")
                .build(),
        ])
        .with_tools([Function::builder()
            .with_name("add_number")
            .with_description("add two numbers")
            .with_parameters(
                Parameters::builder()
                    .add_property(
                        "a",
                        ParameterProperty::builder()
                            .with_description("number 1 in 2 numbers")
                            .with_type(ParameterType::number)
                            .build()?,
                        true,
                    )
                    .add_property(
                        "b",
                        ParameterProperty::builder()
                            .with_description("number 2 in 2 numbers")
                            .with_type(ParameterType::number)
                            .build()?,
                        true,
                    )
                    .build()?,
            )
            .build()?])
        .with_stream(false) // if true, the response will be a stream
        .build()?;

    // call request
    let res = req.call(&client, None).await?;

    // base on with_stream, the rep will be different
    let rep = match res {
        // will return result at once
        ChatCompletionResult::Response(rep) => rep,
        // will return a async receiver of ChatCompletionStreamData
        ChatCompletionResult::Delta(mut rx) => {
            let mut rep_total = ChatCompletionResponse::default();
            while let Some(res) = rx.recv().await {
                match res {
                    Ok(rep) => {
                        rep_total.merge_delta(rep);
                    }
                    Err(e) => {
                        error!("failed to recv rep: {:?}", e);
                        break;
                    }
                }
            }
            rep_total
        }
    };

    // log and print result
    for l in serde_json::to_string_pretty(&rep)?.lines() {
        info!("FINAL REP: {}", l);
    }

    Ok(())
}

#[cfg(test)]
#[tokio::test]
async fn test_chat_simple_ok() -> Result<()> {
    let client = Client::from_env_file(".env.stepfun")?;

    let model_name = std::env::var("OPENAI_API_MODEL_NAME")?;
    let use_stream = std::env::var("USE_STREAM").is_ok();

    let _ = tracing_subscriber::fmt::try_init();

    let req = ChatCompletionRequest::builder()
        .with_model("step-1-8k")
        .with_messages([
            Message::builder()
                .with_role(Role::system)
                .with_content("you are a good llm model")
                .build(),
            Message::builder()
                .with_role(Role::user)
                .with_content("calculate 1921.23 + 42.00")
                .build(),
        ])
        .with_tools([Function::builder()
            .with_name("add_number")
            .with_description("add two numbers")
            .with_parameters(
                Parameters::builder()
                    .add_property(
                        "a",
                        ParameterProperty::builder()
                            .with_description("number 1 in 2 numbers")
                            .with_type(ParameterType::number)
                            .build()?,
                        true,
                    )
                    .add_property(
                        "b",
                        ParameterProperty::builder()
                            .with_description("number 2 in 2 numbers")
                            .with_type(ParameterType::number)
                            .build()?,
                        true,
                    )
                    .build()?,
            )
            .build()?])
        .with_stream(false) // if true, the response will be a stream
        .build()?;

    let res = req.call(&client, None).await?;

    let rep = match res {
        ChatCompletionResult::Response(rep) => rep,
        ChatCompletionResult::Delta(mut rx) => {
            let mut rep_total = ChatCompletionResponse::default();
            while let Some(res) = rx.recv().await {
                match res {
                    Ok(rep) => {
                        rep_total.merge_delta(rep);
                    }
                    Err(e) => {
                        error!("failed to recv rep: {:?}", e);
                        break;
                    }
                }
            }
            rep_total
        }
    };

    for l in serde_json::to_string_pretty(&rep)?.lines() {
        info!("FINAL REP: {}", l);
    }

    Ok(())
}
