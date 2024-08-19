#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]

/// authorization module
pub mod auth;

/// client to execute RESTful API
pub mod client;

/// error module
pub mod error;

/// proto module
pub mod proto;
#[cfg(test)]
pub mod tests;

/// convinent prelude to import module
pub mod prelude {
    pub use crate::auth::*;
    pub use crate::client::*;
    pub use crate::error::*;
    pub use crate::proto::*;
}
