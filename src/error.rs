use serde::Deserialize;

/// Convenience alias for a `Result` whose error is [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// A SendByte error.
///
/// Every failure the SDK surfaces carries the API's error shape: a machine
/// readable [`code`](Error::code), a human readable [`message`](Error::message),
/// an optional [`docs_url`](Error::docs_url), and, for failures that reached the
/// API, the HTTP [`status`](Error::status).
#[derive(Debug, Clone)]
pub struct Error {
    /// Machine readable error code, e.g. `invalid_api_key` or `rate_limited`.
    pub code: String,
    /// Human readable explanation.
    pub message: String,
    /// Link to the relevant documentation, when the API provides one.
    pub docs_url: Option<String>,
    /// HTTP status code, when the failure reached the API. `None` for local
    /// validation and transport errors.
    pub status: Option<u16>,
}

impl Error {
    pub(crate) fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            docs_url: None,
            status: None,
        }
    }

    /// Wraps a transport failure (timeout, DNS, connection refused) as a
    /// `request_failed` error.
    pub(crate) fn transport(err: &reqwest::Error) -> Self {
        Self::new(
            "request_failed",
            format!("Could not reach the SendByte API: {err}"),
        )
    }

    pub(crate) fn invalid_api_key() -> Self {
        Self::new(
            "invalid_api_key",
            "API key must start with sk_live_ or sk_test_. Create one in the dashboard under API keys.",
        )
    }

    pub(crate) fn invalid_live_key() -> Self {
        Self::new(
            "invalid_api_key",
            "API key must start with sk_live_ to use the live client. Create one in the dashboard.",
        )
    }

    pub(crate) fn invalid_sandbox_key() -> Self {
        Self::new(
            "invalid_api_key",
            "API key must start with sk_test_ to use the sandbox client. Create one in the dashboard.",
        )
    }

    pub(crate) fn insecure_base_url() -> Self {
        Self::new(
            "insecure_base_url",
            "Refusing to send a live API key (sk_live_) over an insecure base URL. Use https, or a test key for local http.",
        )
    }

    /// Reports whether this error is a retryable HTTP status (429 or 5xx).
    pub fn is_retryable(&self) -> bool {
        self.status.map(is_retryable_status).unwrap_or(false)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sendbyte: {} ({})", self.message, self.code)
    }
}

impl std::error::Error for Error {}

pub(crate) fn is_retryable_status(status: u16) -> bool {
    status == 429 || (500..600).contains(&status)
}

/// Turns a non-2xx response body into a typed [`Error`], never failing on a
/// non-JSON body (a gateway or proxy 502/504 may return HTML or plain text).
pub(crate) fn parse_error(body: &[u8], status: u16) -> Error {
    #[derive(Deserialize)]
    struct Wrapper {
        error: Option<ApiError>,
    }
    #[derive(Deserialize)]
    struct ApiError {
        code: Option<String>,
        message: Option<String>,
        docs_url: Option<String>,
    }

    match serde_json::from_slice::<Wrapper>(body) {
        Ok(Wrapper { error: Some(e) }) if e.code.is_some() || e.message.is_some() => Error {
            code: e.code.unwrap_or_else(|| "request_failed".into()),
            message: e
                .message
                .unwrap_or_else(|| format!("Request failed with status {status}")),
            docs_url: e.docs_url,
            status: Some(status),
        },
        _ => Error {
            code: "request_failed".into(),
            message: format!("Request failed with status {status}"),
            docs_url: None,
            status: Some(status),
        },
    }
}
