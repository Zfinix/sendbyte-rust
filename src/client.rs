use std::sync::Arc;
use std::time::{Duration, SystemTime};

use reqwest::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::domains::Domains;
use crate::emails::Emails;
use crate::error::{is_retryable_status, parse_error, Error, Result};
use crate::webhooks::Webhooks;

const DEFAULT_BASE_URL: &str = "https://api.sendbyte.africa";
pub(crate) const VERSION: &str = "0.2.0";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_MAX_ATTEMPTS: u32 = 3;
const MAX_BACKOFF: Duration = Duration::from_secs(20);

/// The SendByte API client.
///
/// A `Client` is cheap to clone (it is a handle around a shared, connection
/// pooled [`reqwest::Client`]) so pass clones around freely rather than building
/// a new one per request.
///
/// ```no_run
/// # async fn run() -> Result<(), sendbyte::Error> {
/// use sendbyte::{Client, SendEmailRequest};
///
/// let client = Client::new("sk_test_...")?;
/// let email = client
///     .emails()
///     .send(
///         SendEmailRequest::new("PayLink <receipts@paylink.ng>", ["amaka@halo.ng"])
///             .subject("Receipt")
///             .html("<p>Thank you.</p>"),
///     )
///     .await?;
/// println!("{}", email.id);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

struct Inner {
    base_url: String,
    http: reqwest::Client,
    max_attempts: u32,
    timeout: Duration,
    // Precomputed once so the hot path never re-allocates per attempt.
    auth: HeaderValue,
    user_agent: HeaderValue,
}

const JSON_CONTENT_TYPE: HeaderValue = HeaderValue::from_static("application/json");

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("base_url", &self.inner.base_url)
            .field("max_attempts", &self.inner.max_attempts)
            .field("timeout", &self.inner.timeout)
            .field("api_key", &"[redacted]")
            .finish()
    }
}

impl Client {
    /// Creates a client. The key must start with `sk_live_` or `sk_test_`.
    ///
    /// Keys that start with `sk_test_` run in the sandbox: every endpoint
    /// behaves the same, but nothing is actually delivered.
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Self::builder(api_key).build()
    }

    /// Creates a sandbox client. The key must start with `sk_test_`.
    pub fn new_sandbox(api_key: impl Into<String>) -> Result<Self> {
        let key = api_key.into();
        if !key.starts_with("sk_test_") {
            return Err(Error::invalid_sandbox_key());
        }
        Self::builder(key).build()
    }

    /// Creates a live client. The key must start with `sk_live_`.
    pub fn new_live(api_key: impl Into<String>) -> Result<Self> {
        let key = api_key.into();
        if !key.starts_with("sk_live_") {
            return Err(Error::invalid_live_key());
        }
        Self::builder(key).build()
    }

    /// Starts a [`ClientBuilder`] to override the base URL, HTTP client, retry
    /// budget, or timeout.
    pub fn builder(api_key: impl Into<String>) -> ClientBuilder {
        ClientBuilder {
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            http: None,
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Access the `/v1/emails` endpoints.
    pub fn emails(&self) -> Emails<'_> {
        Emails { client: self }
    }

    /// Access the `/v1/domains` endpoints.
    pub fn domains(&self) -> Domains<'_> {
        Domains { client: self }
    }

    /// Access the `/v1/webhooks` endpoints.
    pub fn webhooks(&self) -> Webhooks<'_> {
        Webhooks { client: self }
    }

    pub(crate) async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        decode(self.execute(Method::GET, path, None).await?)
    }

    pub(crate) async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let payload =
            serde_json::to_vec(body).map_err(|e| Error::new("invalid_request", e.to_string()))?;
        decode(self.execute(Method::POST, path, Some(payload)).await?)
    }

    pub(crate) async fn post_empty<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        decode(self.execute(Method::POST, path, None).await?)
    }

    pub(crate) async fn delete_discard(&self, path: &str) -> Result<()> {
        let body = self.execute(Method::DELETE, path, None).await?;
        if body.is_empty() {
            return Ok(());
        }
        serde_json::from_slice::<serde::de::IgnoredAny>(&body)
            .map(|_| ())
            .map_err(|e| Error::new("invalid_response", e.to_string()))
    }

    /// Runs a request with retries on 429, 5xx, and transport errors, using
    /// exponential backoff with full jitter and honoring `Retry-After`. Returns
    /// the raw success body bytes.
    async fn execute(&self, method: Method, path: &str, body: Option<Vec<u8>>) -> Result<Vec<u8>> {
        let url = format!("{}{}", self.inner.base_url, path);
        let mut last_err: Option<Error> = None;

        for attempt in 1..=self.inner.max_attempts {
            let mut req = self
                .inner
                .http
                .request(method.clone(), &url)
                .header(AUTHORIZATION, self.inner.auth.clone())
                .header(USER_AGENT, self.inner.user_agent.clone())
                .timeout(self.inner.timeout);
            if let Some(payload) = &body {
                req = req.header(CONTENT_TYPE, JSON_CONTENT_TYPE).body(payload.clone());
            }

            let res = match req.send().await {
                Ok(res) => res,
                Err(err) => {
                    last_err = Some(Error::transport(&err));
                    if attempt < self.inner.max_attempts {
                        sleep(backoff_delay(attempt, None)).await;
                        continue;
                    }
                    return Err(last_err.unwrap());
                }
            };

            let status = res.status().as_u16();
            let retry_after = res
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            let bytes = match res.bytes().await {
                Ok(b) => b.to_vec(),
                Err(err) => {
                    last_err = Some(Error::transport(&err));
                    if attempt < self.inner.max_attempts {
                        sleep(backoff_delay(attempt, None)).await;
                        continue;
                    }
                    return Err(last_err.unwrap());
                }
            };

            if status >= 400 {
                if is_retryable_status(status) && attempt < self.inner.max_attempts {
                    last_err = Some(parse_error(&bytes, status));
                    sleep(backoff_delay(attempt, retry_after.as_deref())).await;
                    continue;
                }
                return Err(parse_error(&bytes, status));
            }

            return Ok(bytes);
        }

        Err(last_err.unwrap_or_else(|| {
            Error::new("request_failed", "Request failed after exhausting retries")
        }))
    }
}

fn decode<T: DeserializeOwned>(body: Vec<u8>) -> Result<T> {
    let slice: &[u8] = if body.is_empty() { b"null" } else { &body };
    serde_json::from_slice(slice).map_err(|e| {
        Error::new(
            "invalid_response",
            format!("The API returned a response that could not be decoded: {e}"),
        )
    })
}

/// Builder for a [`Client`]. Obtain one from [`Client::builder`].
pub struct ClientBuilder {
    api_key: String,
    base_url: String,
    http: Option<reqwest::Client>,
    max_attempts: u32,
    timeout: Duration,
}

impl ClientBuilder {
    /// Overrides the API host (staging, self-hosted, or local development).
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into().trim_end_matches('/').to_string();
        self
    }

    /// Injects a custom [`reqwest::Client`], e.g. to share a connection pool or
    /// configure a proxy.
    pub fn http_client(mut self, http: reqwest::Client) -> Self {
        self.http = Some(http);
        self
    }

    /// Caps the total attempts per request, including the first (so `3` means up
    /// to 2 retries). Values below 1 are treated as 1.
    pub fn max_attempts(mut self, n: u32) -> Self {
        self.max_attempts = n.max(1);
        self
    }

    /// Sets the per-attempt timeout. Defaults to 30 seconds.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Validates the key and base URL and builds the [`Client`].
    pub fn build(self) -> Result<Client> {
        if !self.api_key.starts_with("sk_live_") && !self.api_key.starts_with("sk_test_") {
            return Err(Error::invalid_api_key());
        }
        // A live key over a non-https base URL would send the secret in cleartext.
        if self.api_key.starts_with("sk_live_") && !is_secure_base_url(&self.base_url) {
            return Err(Error::insecure_base_url());
        }
        let mut auth = HeaderValue::from_str(&format!("Bearer {}", self.api_key))
            .map_err(|_| Error::invalid_api_key())?;
        // Keep the key out of any header-dumping logs.
        auth.set_sensitive(true);
        let user_agent = HeaderValue::from_str(&format!("sendbyte-rust/{VERSION}"))
            .expect("user agent is always a valid header value");

        let http = self.http.unwrap_or_default();
        Ok(Client {
            inner: Arc::new(Inner {
                base_url: self.base_url,
                http,
                max_attempts: self.max_attempts,
                timeout: self.timeout,
                auth,
                user_agent,
            }),
        })
    }
}

/// https is required; http is tolerated only for localhost/loopback dev hosts.
fn is_secure_base_url(base_url: &str) -> bool {
    match reqwest::Url::parse(base_url) {
        Ok(url) => {
            if url.scheme() == "https" {
                return true;
            }
            matches!(
                url.host_str(),
                Some("localhost" | "127.0.0.1" | "::1" | "[::1]")
            )
        }
        Err(_) => false,
    }
}

/// Exponential backoff with full jitter, capped at [`MAX_BACKOFF`], honoring
/// `Retry-After` (delta-seconds or an HTTP-date) when present. `attempt` is
/// 1-based.
fn backoff_delay(attempt: u32, retry_after: Option<&str>) -> Duration {
    if let Some(delay) = retry_after.and_then(parse_retry_after) {
        return delay.min(MAX_BACKOFF);
    }
    let shift = (attempt - 1).min(20);
    let base_ms = 250u64
        .saturating_mul(1u64 << shift)
        .min(MAX_BACKOFF.as_millis() as u64);
    Duration::from_millis(fastrand::u64(0..=base_ms))
}

fn parse_retry_after(value: &str) -> Option<Duration> {
    let value = value.trim();
    if let Ok(secs) = value.parse::<i64>() {
        return Some(Duration::from_secs(secs.max(0) as u64));
    }
    if let Ok(when) = httpdate::parse_http_date(value) {
        return Some(
            when.duration_since(SystemTime::now())
                .unwrap_or(Duration::ZERO),
        );
    }
    None
}

async fn sleep(d: Duration) {
    if d > Duration::ZERO {
        tokio::time::sleep(d).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secure_base_url_rules() {
        assert!(is_secure_base_url("https://api.sendbyte.africa"));
        assert!(is_secure_base_url("http://localhost:8080"));
        assert!(is_secure_base_url("http://127.0.0.1:3000"));
        assert!(!is_secure_base_url("http://api.sendbyte.africa"));
        assert!(!is_secure_base_url("not a url"));
    }

    #[test]
    fn backoff_is_capped_and_honors_retry_after() {
        assert_eq!(backoff_delay(1, Some("2")), Duration::from_secs(2));
        assert_eq!(backoff_delay(1, Some("999")), MAX_BACKOFF);
        for attempt in 1..=10 {
            assert!(backoff_delay(attempt, None) <= MAX_BACKOFF);
        }
    }

    #[test]
    fn live_key_requires_https() {
        let err = Client::builder("sk_live_abc")
            .base_url("http://api.sendbyte.africa")
            .build()
            .unwrap_err();
        assert_eq!(err.code, "insecure_base_url");
    }

    #[test]
    fn rejects_bad_key_prefix() {
        let err = Client::new("bogus").unwrap_err();
        assert_eq!(err.code, "invalid_api_key");
    }
}
