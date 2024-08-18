use std::collections::HashMap;

use crate::error::*;

use base64::Engine;
use smart_default::SmartDefault;
use tracing::*;

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

    pub fn with_messages(mut self, messages: Vec<Message>) -> Self {
        self.messages = messages;
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

    pub fn with_tools(mut self, tools: Vec<ToolCall>) -> Self {
        self.tools = tools;
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
            Some(lhs) => Some(lhs.add(rhs)),
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

        let model = model.ok_or(Error::ChatCompletionsRequestBuilderMissModel)?;

        if messages.is_empty() {
            return Err(Error::ChatCompletionsRequestBuilderMissMessages);
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
        info!("merging delta: {:?}", delta);
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
                        choice_content = match choice_content {
                            Some(c) => Some(c.merge(delta_content)),
                            None => Some(delta_content),
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
                                if rhs.function.name.is_none() {
                                    lhs.function.name = rhs.function.name.clone();
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Choice {
    pub index: usize,
    pub message: Message,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Stop {
    Text(String),
    Texts(Vec<String>),
}

impl Stop {
    pub fn add(self, rhs: Stop) -> Self {
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, SmartDefault)]
pub struct ChatComplitionUsage {
    pub cached_tokens: Option<u64>,
    pub completion_tokens: u64,
    pub prompt_tokens: u64,
    pub total_tokens: u64,
}

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
    use serde::de::{Deserialize, IntoDeserializer};
    let opt = Option::<String>::deserialize(de)?;
    let opt = opt.as_ref().map(String::as_str);
    match opt {
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

    pub fn add_content(mut self, content: impl Into<Content>) -> Self {
        let mut lhs = None;
        std::mem::swap(&mut self.content, &mut lhs);
        self.content = match lhs {
            None => Some(content.into()),
            Some(lhs) => Some(lhs.merge(content)),
        };
        self
    }

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(non_camel_case_types)]
pub enum Role {
    system,
    user,
    assistant,
    tool,
}

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
            typ: "image".into(),
            image_url: ImageUrl::from_url(url),
        }])
    }

    pub fn from_text(text: impl Into<String>) -> Self {
        Content::Text(text.into())
    }

    pub fn merge(self, rhs: impl Into<Self>) -> Self {
        let rhs = rhs.into();
        let lhs = self;
        match (lhs, rhs) {
            (Content::Text(lhs), Content::Text(rhs)) => Content::Text(format!("{}{}", lhs, rhs)),
            (Content::Containers(mut lhs), Content::Containers(rhs)) => {
                lhs.extend(rhs);
                Content::Containers(lhs)
            }
            (Content::Text(lhs), Content::Containers(mut rhs)) => {
                let mut new = vec![ContentContainer::Text {
                    typ: "text".into(),
                    text: lhs,
                }];
                new.append(&mut rhs);
                Content::Containers(new)
            }
            (Content::Containers(mut lhs), Content::Text(rhs)) => {
                lhs.push(ContentContainer::Text {
                    typ: "text".into(),
                    text: rhs,
                });
                Content::Containers(lhs)
            }
        }
    }
}

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImageUrl {
    pub url: String,
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub typ: Option<String>,
    pub function: Function,
}

impl From<Function> for ToolCall {
    fn from(f: Function) -> Self {
        ToolCall {
            id: None,
            typ: Some("function".to_string()),
            function: f,
        }
    }
}

impl ToolCall {
    pub fn builder() -> ToolCallBuilder {
        ToolCallBuilder::default()
    }
}

#[derive(Debug, Clone, SmartDefault)]
pub struct ToolCallBuilder {
    pub id: Option<String>,
    #[default(Some("function".to_string()))]
    typ: Option<String>,
    function: Option<Function>,
}

impl ToolCallBuilder {
    pub fn with_function(mut self, function: impl Into<Function>) -> Self {
        self.function = Some(function.into());
        self
    }

    pub fn build(self) -> Result<ToolCall> {
        let Self { id, typ, function } = self;
        let typ = typ.ok_or(Error::ToolCallBuild)?;
        let function = function.ok_or(Error::ToolCallBuild)?;
        Ok(ToolCall {
            id,
            typ: Some(typ),
            function,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Function {
    pub name: Option<String>,
    pub description: Option<String>,
    pub parameters: Option<Parameters>,
    pub arguments: Option<String>,
}

pub mod serde_value {

    use serde::de::{self, Deserialize, DeserializeOwned, Deserializer};
    use serde::ser::{self, Serialize, Serializer};
    use serde_json;

    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Serialize,
        S: Serializer,
    {
        let j = serde_json::to_string(value).map_err(ser::Error::custom)?;
        j.serialize(serializer)
    }

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: DeserializeOwned,
        D: Deserializer<'de>,
    {
        let j = String::deserialize(deserializer)?;
        serde_json::from_str(&j).map_err(de::Error::custom)
    }
}

impl Function {
    pub fn builder() -> FunctionBuilder {
        FunctionBuilder::default()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, SmartDefault)]
pub struct FunctionBuilder {
    pub name: Option<String>,
    pub description: Option<String>,
    pub parameters: Option<Parameters>,
    pub arguments: Option<String>,
}

impl FunctionBuilder {
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_parameters(mut self, parameters: Parameters) -> Self {
        self.parameters = Some(parameters);
        self
    }

    pub fn build(self) -> Result<Function> {
        let Self {
            name,
            description,
            parameters,
            arguments,
        } = self;

        let name = name.ok_or(Error::ToolCallFunctionBuild)?;

        Ok(Function {
            name: Some(name),
            description,
            parameters,
            arguments,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(non_camel_case_types)]
#[serde(untagged)]
pub enum Argument {
    string(String),
    number(f64),
    integer(i64),
    boolean(bool),
    array(Vec<Argument>),
    object(HashMap<String, Argument>),
}

macro_rules! impl_argument_as_value {
    ($fun: ident, $i: ident, $typ: ty) => {
        impl Argument {
            pub fn $fun(&self) -> Option<&$typ> {
                match self {
                    Self::$i(inner) => Some(&inner),
                    _ => None,
                }
            }
        }
    };
}

impl_argument_as_value!(as_string, string, String);
impl_argument_as_value!(as_number, number, f64);
impl_argument_as_value!(as_integer, integer, i64);
impl_argument_as_value!(as_boolean, boolean, bool);
impl_argument_as_value!(as_array, array, Vec<Argument>);
impl_argument_as_value!(as_object, object, HashMap<String, Argument>);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Parameters {
    #[serde(rename = "type")]
    pub typ: String,
    pub properties: HashMap<String, ParameterProperty>,
    #[serde(default)]
    pub required: Vec<String>,
}

impl Parameters {
    pub fn builder() -> ParametersBuilder {
        ParametersBuilder::default()
    }
}

#[derive(Debug, Clone, SmartDefault)]
pub struct ParametersBuilder {
    #[default(Some("object".to_string()))]
    typ: Option<String>,
    properties: HashMap<String, ParameterProperty>,
    required: Vec<String>,
}

impl ParametersBuilder {
    pub fn add_property(mut self, name: impl Into<String>, property: ParameterProperty) -> Self {
        self.properties.insert(name.into(), property);
        self
    }

    pub fn add_required(mut self, name: impl Into<String>) -> Self {
        self.required.push(name.into());
        self
    }

    pub fn build(self) -> Result<Parameters> {
        let Self {
            typ,
            properties,
            required,
        } = self;

        let typ = typ.ok_or(Error::ToolCallParametersBuild)?;

        Ok(Parameters {
            typ,
            properties,
            required,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ParameterProperty {
    #[serde(rename = "type")]
    pub typ: Option<ParameterType>,
    pub description: String,
}

impl ParameterProperty {
    pub fn builder() -> ParameterPropertyBuilder {
        ParameterPropertyBuilder::default()
    }
}

#[derive(Debug, Clone, SmartDefault)]
pub struct ParameterPropertyBuilder {
    typ: Option<ParameterType>,
    description: Option<String>,
}

impl ParameterPropertyBuilder {
    pub fn with_type(mut self, typ: ParameterType) -> Self {
        self.typ = Some(typ);
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn build(self) -> Result<ParameterProperty> {
        let Self { typ, description } = self;

        let typ = typ.ok_or(Error::ToolCallParametersBuild)?;
        let description = description.ok_or(Error::ToolCallParametersBuild)?;

        Ok(ParameterProperty {
            typ: Some(typ),
            description,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(non_camel_case_types)]
pub enum ParameterType {
    string,
    number,
    integer,
    boolean,
    array,
    object,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatCompletionStreamData {
    pub id: Option<String>,
    pub object: Option<String>,
    pub created: Option<u64>,
    pub model: Option<String>,
    pub choices: Vec<StreamChoice>,
    pub usage: Option<ChatComplitionUsage>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StreamChoice {
    pub index: usize,
    pub delta: Message,
    pub finish_reason: Option<String>,
    pub usage: Option<ChatComplitionUsage>,
}

#[cfg(test)]
#[test]
fn test_message_ok() -> anyhow::Result<()> {
    let _ = dotenv::from_filename(".env.stepfun")?;
    let _ = tracing_subscriber::fmt::try_init();

    let req: ChatCompletionRequest =
        serde_json::from_str(&crate::tests::STEPFUN_CHAT_COMPLETION_REQUEST_JSON)?;

    for l in serde_json::to_string_pretty(&req)?.lines() {
        tracing::info!("REQ: {}", l);
    }

    let tools: Vec<ToolCall> =
        serde_json::from_str(&crate::tests::STEPFUN_CHAT_TOOLS_REQUEST_JSON)?;

    for l in serde_json::to_string_pretty(&tools)?.lines() {
        tracing::info!("TOOLS: {}", l);
    }

    let rep: ChatCompletionResponse =
        serde_json::from_str(&crate::tests::STEPFUN_CHAT_TOOLS_RESPONSE_JSON)?;

    for l in serde_json::to_string_pretty(&rep)?.lines() {
        tracing::info!("TOOLS RESPONSE: {}", l);
    }

    let tools: Vec<ToolCall> = serde_json::from_str(&crate::tests::KIMI_CHAT_TOOL_JSON)?;

    for l in serde_json::to_string_pretty(&tools)?.lines() {
        tracing::info!("KIMI TOOLS RESPONSE: {}", l);
    }

    Ok(())
}
