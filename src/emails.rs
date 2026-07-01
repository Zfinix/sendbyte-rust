use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::Result;

/// The `/v1/emails` endpoints. Obtain one from [`Client::emails`].
pub struct Emails<'a> {
    pub(crate) client: &'a Client,
}

impl Emails<'_> {
    /// Queues an email for delivery.
    ///
    /// If the request has no `idempotency_key`, one is generated and sent, so a
    /// retried send (whether by the SDK's own retry loop or by your code) cannot
    /// double-send.
    pub async fn send(&self, mut req: SendEmailRequest) -> Result<Email> {
        if req.idempotency_key.is_none() {
            req.idempotency_key = Some(uuid::Uuid::new_v4().to_string());
        }
        self.client.post("/v1/emails", &req).await
    }

    /// Fetches one email with its event timeline.
    pub async fn get(&self, id: &str) -> Result<EmailDetail> {
        self.client.get(&format!("/v1/emails/{id}")).await
    }

    /// Pages through the email log, newest first.
    pub async fn list(&self, params: &ListEmailsParams) -> Result<EmailList> {
        let mut query: Vec<(&str, String)> = Vec::new();
        if let Some(limit) = params.limit {
            query.push(("limit", limit.to_string()));
        }
        if let Some(after) = &params.after {
            query.push(("after", after.clone()));
        }
        if let Some(status) = &params.status {
            query.push(("status", status.clone()));
        }
        let path = if query.is_empty() {
            "/v1/emails".to_string()
        } else {
            let qs: Vec<String> = query
                .iter()
                .map(|(k, v)| format!("{k}={}", urlencode(v)))
                .collect();
            format!("/v1/emails?{}", qs.join("&"))
        };
        self.client.get(&path).await
    }
}

fn urlencode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// A file to attach to an email. `content` is base64-encoded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub filename: String,
    pub content: String,
    pub content_type: String,
}

/// One-click unsubscribe details (RFC 8058).
///
/// Provide an https `url` and/or a `mailto`. The API emits `List-Unsubscribe`
/// and, for an https url, `List-Unsubscribe-Post` so Gmail and Yahoo render a
/// native unsubscribe. Recommended for bulk mail under the 2024 sender rules;
/// harmless on transactional.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListUnsubscribe {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mailto: Option<String>,
}

/// The payload for [`Emails::send`].
///
/// Build one with [`SendEmailRequest::new`] and the chainable setters, or
/// construct the struct directly (all fields are public).
#[derive(Debug, Clone, Default, Serialize)]
pub struct SendEmailRequest {
    pub from: String,
    pub to: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cc: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub bcc: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reply_to: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<Attachment>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_unsubscribe: Option<ListUnsubscribe>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheduled_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

impl SendEmailRequest {
    /// Creates a request with the required `from` and `to` fields.
    ///
    /// `to` accepts anything iterable of strings, e.g. a single address wrapped
    /// in an array (`["amaka@halo.ng"]`) or a `Vec<String>`.
    pub fn new<I, S>(from: impl Into<String>, to: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            from: from.into(),
            to: to.into_iter().map(Into::into).collect(),
            ..Default::default()
        }
    }

    /// Sets the subject line.
    pub fn subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    /// Sets the HTML body.
    pub fn html(mut self, html: impl Into<String>) -> Self {
        self.html = Some(html.into());
        self
    }

    /// Sets the plain-text body.
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Sets the CC recipients.
    pub fn cc<I, S>(mut self, cc: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.cc = cc.into_iter().map(Into::into).collect();
        self
    }

    /// Sets the BCC recipients.
    pub fn bcc<I, S>(mut self, bcc: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.bcc = bcc.into_iter().map(Into::into).collect();
        self
    }

    /// Sets the Reply-To addresses.
    pub fn reply_to<I, S>(mut self, reply_to: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.reply_to = reply_to.into_iter().map(Into::into).collect();
        self
    }

    /// Adds one attachment.
    pub fn attachment(mut self, attachment: Attachment) -> Self {
        self.attachments.push(attachment);
        self
    }

    /// Sets the tags.
    pub fn tags<I, S>(mut self, tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    /// Adds one custom header.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }

    /// Sets one-click unsubscribe details.
    pub fn list_unsubscribe(mut self, list_unsubscribe: ListUnsubscribe) -> Self {
        self.list_unsubscribe = Some(list_unsubscribe);
        self
    }

    /// Schedules the send for a future ISO 8601 timestamp.
    pub fn scheduled_at(mut self, scheduled_at: impl Into<String>) -> Self {
        self.scheduled_at = Some(scheduled_at.into());
        self
    }

    /// Renders a server-side template with the given variables.
    pub fn template(
        mut self,
        template_id: impl Into<String>,
        variables: serde_json::Value,
    ) -> Self {
        self.template_id = Some(template_id.into());
        self.variables = Some(variables);
        self
    }

    /// Sets an explicit idempotency key. When unset, [`Emails::send`] generates
    /// one for you.
    pub fn idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }
}

/// A summary of an email in the log.
#[derive(Debug, Clone, Deserialize)]
pub struct Email {
    pub id: String,
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,
    /// One of `queued`, `sending`, `sent`, `delivered`, `bounced`, `complained`,
    /// or `suppressed`. Kept as a string so new statuses do not break decoding.
    pub status: String,
    pub sandbox: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    /// The delivered RFC Message-ID once known (`None` until sent). Use it as the
    /// threading anchor for `In-Reply-To` / `References` on follow-up sends.
    pub message_id: Option<String>,
    pub scheduled_at: Option<String>,
    pub created_at: String,
}

/// One lifecycle event on an email.
#[derive(Debug, Clone, Deserialize)]
pub struct EmailEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub payload: serde_json::Value,
    pub created_at: String,
}

/// The full email record including content and events.
#[derive(Debug, Clone, Deserialize)]
pub struct EmailDetail {
    #[serde(flatten)]
    pub email: Email,
    pub html: Option<String>,
    pub text: Option<String>,
    #[serde(default)]
    pub events: Vec<EmailEvent>,
}

/// A page of the email log.
#[derive(Debug, Clone, Deserialize)]
pub struct EmailList {
    pub data: Vec<Email>,
    pub has_more: bool,
}

/// Filters for [`Emails::list`].
#[derive(Debug, Clone, Default)]
pub struct ListEmailsParams {
    pub limit: Option<u32>,
    pub after: Option<String>,
    pub status: Option<String>,
}

impl ListEmailsParams {
    /// Creates an empty filter (equivalent to [`Default`]).
    pub fn new() -> Self {
        Self::default()
    }

    /// Caps the page size.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Pages after the given email id (cursor).
    pub fn after(mut self, after: impl Into<String>) -> Self {
        self.after = Some(after.into());
        self
    }

    /// Filters to a single status.
    pub fn status(mut self, status: impl Into<String>) -> Self {
        self.status = Some(status.into());
        self
    }
}
