#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("message builder without role")]
    MessageBuilderMissRole,
    #[error("message builder without messages")]
    MessageBuilderMissMessages,
    #[error("json encode/decode fail: {0}")]
    JsonDecode(#[from] serde_json::Error),
    #[error("build header value error: {0}")]
    HeaderValue(#[from] http::header::InvalidHeaderValue),
    #[error("parse url string: {0}")]
    UrlParse(#[from] url::ParseError),
    #[error("client builder without base url")]
    ClientBuilderMissBaseUrl,
    #[error("client builder without authenticator")]
    ClientBuilderMissAuthenticator,
    #[error("client failed to build request")]
    ClientBuildRequest(#[from] reqwest::Error),
    #[cfg(feature = "opencv")]
    #[error("failed to process image with opencv: {0}")]
    Opencv(#[from] opencv::Error),
    #[error("chat completion builder without model")]
    ChatCompletionsRequestBuilderMissModel,
    #[error("chat completion builder without message")]
    ChatCompletionsRequestBuilderMissMessages,
    #[error("failed to decode utf-8: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("failed to send message to chat reciever")]
    SendMessage,
    #[error("read or write file {0}")]
    Io(#[from] std::io::Error),
    #[error("no file name")]
    NoFileName,
    #[error("no file extension found")]
    NoFileExtension,
    #[error("failed to join async task")]
    JoinError(#[from] tokio::task::JoinError),
    #[error("failed to build tool call")]
    ToolCallBuild,
    #[error("failed to build tool call parameters")]
    ToolCallParametersBuild,
    #[error("failed to build tool call function")]
    ToolCallFunctionBuild,
    #[error("failed to build generation request")]
    GenerationRequestBuild,
    #[error("api server error code={0}")]
    ApiError(u16),
    #[error("failed to build file request")]
    FileRequestBuild,
    #[error("failed to find env var")]
    Var(#[from] std::env::VarError),
}

pub type Result<T> = std::result::Result<T, Error>;
