use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("credentials unavailable: {0}")]
    Credentials(String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("AWS {status} {code}: {message}{request_suffix}")]
    Aws {
        status: u16,
        code: String,
        message: String,
        request_id: Option<String>,
        request_suffix: String,
    },
    #[error("HTTP transport: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JSON protocol: {0}")]
    Json(#[from] serde_json::Error),
    #[error("XML protocol: {0}")]
    Xml(#[from] quick_xml::DeError),
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    pub fn aws(
        status: u16,
        code: impl Into<String>,
        message: impl Into<String>,
        request_id: Option<String>,
    ) -> Self {
        let request_suffix = request_id
            .as_deref()
            .map(|id| format!(" (request {id})"))
            .unwrap_or_default();
        Self::Aws {
            status,
            code: code.into(),
            message: message.into(),
            request_id,
            request_suffix,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
