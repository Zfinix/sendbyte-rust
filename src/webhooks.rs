use serde::{Deserialize, Deserializer, Serialize};

use crate::client::Client;
use crate::error::Result;

/// The `/v1/webhooks` endpoints. Obtain one from [`Client::webhooks`].
pub struct Webhooks<'a> {
    pub(crate) client: &'a Client,
}

impl Webhooks<'_> {
    /// Registers an endpoint. Pass `None` for `events` to receive everything.
    ///
    /// The returned [`WebhookEndpoint::secret`] is shown only here; store it now.
    pub async fn create(
        &self,
        url: impl Into<String>,
        events: Option<Vec<String>>,
    ) -> Result<WebhookEndpoint> {
        #[derive(Serialize)]
        struct Body {
            url: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            events: Option<Vec<String>>,
        }
        self.client
            .post(
                "/v1/webhooks",
                &Body {
                    url: url.into(),
                    events,
                },
            )
            .await
    }

    /// Returns all endpoints (never their secrets).
    pub async fn list(&self) -> Result<Vec<WebhookEndpoint>> {
        let page: WebhookList = self.client.get("/v1/webhooks").await?;
        Ok(page.data)
    }

    /// Stops deliveries to an endpoint.
    pub async fn disable(&self, id: &str) -> Result<()> {
        self.client
            .delete_discard(&format!("/v1/webhooks/{id}"))
            .await
    }

    /// Re-enqueues a past delivery by its id.
    pub async fn replay(&self, delivery_id: &str) -> Result<WebhookReplay> {
        self.client
            .post_empty(&format!("/v1/webhooks/deliveries/{delivery_id}/replay"))
            .await
    }
}

/// The set of events an endpoint subscribes to.
///
/// The API sends either the sentinel string `"all"` or a JSON array of event
/// names; this enum normalizes both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookEvents {
    /// The endpoint receives every event.
    All,
    /// The endpoint receives only these named events.
    Only(Vec<String>),
}

impl WebhookEvents {
    /// Whether the endpoint subscribes to every event.
    pub fn is_all(&self) -> bool {
        matches!(self, WebhookEvents::All)
    }

    /// The subscribed event names, or an empty slice when subscribed to all.
    pub fn names(&self) -> &[String] {
        match self {
            WebhookEvents::All => &[],
            WebhookEvents::Only(names) => names,
        }
    }
}

impl<'de> Deserialize<'de> for WebhookEvents {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Sentinel(String),
            List(Vec<String>),
        }
        Ok(match Raw::deserialize(deserializer)? {
            Raw::Sentinel(s) if s == "all" => WebhookEvents::All,
            Raw::Sentinel(s) => WebhookEvents::Only(vec![s]),
            Raw::List(list) => WebhookEvents::Only(list),
        })
    }
}

/// A registered webhook destination.
#[derive(Debug, Clone, Deserialize)]
pub struct WebhookEndpoint {
    pub id: String,
    pub url: String,
    pub events: WebhookEvents,
    #[serde(default)]
    pub disabled: bool,
    /// When the endpoint was auto-disabled after chronic delivery failures, else
    /// `None`.
    pub disabled_at: Option<String>,
    /// Consecutive failed deliveries; resets to 0 on a success. Watch it to warn
    /// before auto-disable.
    #[serde(default)]
    pub consecutive_failures: i64,
    pub created_at: String,
    /// Present only on creation. Store it; it is never shown again.
    #[serde(default)]
    pub secret: Option<String>,
}

/// The result of replaying a past delivery.
#[derive(Debug, Clone, Deserialize)]
pub struct WebhookReplay {
    pub id: String,
    pub replay_of: String,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize)]
struct WebhookList {
    #[serde(default)]
    data: Vec<WebhookEndpoint>,
}
