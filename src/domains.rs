use serde::Deserialize;

use crate::client::Client;
use crate::error::Result;

/// The `/v1/domains` endpoints. Obtain one from [`Client::domains`].
pub struct Domains<'a> {
    pub(crate) client: &'a Client,
}

impl Domains<'_> {
    /// Registers a domain and returns the DNS records to publish.
    pub async fn create(&self, domain: &str) -> Result<Domain> {
        self.client
            .post("/v1/domains", &serde_json::json!({ "domain": domain }))
            .await
    }

    /// Returns all domains in the project.
    pub async fn list(&self) -> Result<Vec<Domain>> {
        let page: DomainList = self.client.get("/v1/domains").await?;
        Ok(page.data)
    }

    /// Fetches one domain by id, including its DNS records.
    pub async fn get(&self, id: &str) -> Result<Domain> {
        self.client.get(&format!("/v1/domains/{id}")).await
    }

    /// Checks the published DNS records.
    pub async fn verify(&self, id: &str) -> Result<DomainVerification> {
        self.client
            .post_empty(&format!("/v1/domains/{id}/verify"))
            .await
    }
}

/// One record to publish for domain verification.
#[derive(Debug, Clone, Deserialize)]
pub struct DnsRecord {
    #[serde(rename = "type")]
    pub record_type: String,
    pub host: String,
    /// The relative name for the DNS provider's Name/Host field (apex = `@`).
    pub name: String,
    pub value: String,
    /// One of `spf`, `dkim`, `dmarc`, `mailfrom_mx`, or `mailfrom_spf`.
    pub purpose: String,
    pub required: bool,
}

/// A sending domain.
#[derive(Debug, Clone, Deserialize)]
pub struct Domain {
    pub id: String,
    pub domain: String,
    /// One of `pending`, `verified`, or `degraded`.
    pub status: String,
    pub dkim_selector: Option<String>,
    #[serde(default)]
    pub dns_records: Vec<DnsRecord>,
    pub verified_at: Option<String>,
    /// Whether the recommended SPF/DMARC/MAIL FROM records are also published.
    /// Distinct from `status`: a `verified` domain (DKIM only) still
    /// underperforms at Yahoo/Microsoft until this is true.
    #[serde(default)]
    pub deliverability_ready: bool,
    pub created_at: String,
}

/// One row of a domain verification check.
#[derive(Debug, Clone, Deserialize)]
pub struct DomainCheck {
    pub purpose: String,
    pub host: String,
    pub required: bool,
    pub pass: bool,
}

/// The result of a verification check.
#[derive(Debug, Clone, Deserialize)]
pub struct DomainVerification {
    pub id: String,
    pub domain: String,
    pub status: String,
    /// Whether SES has confirmed the identity. Lets callers tell "DNS passed but
    /// SES not yet confirmed" (`Some(false)`) from fully verified (`Some(true)`);
    /// `None` when the API does not report it.
    pub ses_verified: Option<bool>,
    #[serde(default)]
    pub deliverability_ready: bool,
    #[serde(default)]
    pub checks: Vec<DomainCheck>,
}

#[derive(Debug, Clone, Deserialize)]
struct DomainList {
    #[serde(default)]
    data: Vec<Domain>,
}
