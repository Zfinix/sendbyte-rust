//! An unofficial Rust SDK for [SendByte](https://sendbyte.africa), the email API
//! for Africa. Not affiliated with or endorsed by SendByte.
//!
//! Every method maps 1:1 onto a documented REST endpoint. Requests retry on 429,
//! 5xx, and transport errors with exponential backoff plus jitter (honoring
//! `Retry-After`), and `POST /v1/emails` reuses a generated idempotency key
//! across retries so a retried send cannot double-send.
//!
//! # Quickstart
//!
//! ```no_run
//! use sendbyte::{Client, SendEmailRequest};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), sendbyte::Error> {
//!     let client = Client::new("sk_test_...")?;
//!
//!     let email = client
//!         .emails()
//!         .send(
//!             SendEmailRequest::new("PayLink <receipts@paylink.ng>", ["amaka@halo.ng"])
//!                 .subject("Receipt for your payment")
//!                 .html("<p>Thank you. Your payment was received.</p>"),
//!         )
//!         .await?;
//!
//!     println!("{}", email.id);
//!     Ok(())
//! }
//! ```
//!
//! Keys that start with `sk_test_` run in the sandbox: every endpoint behaves
//! the same, but nothing is actually delivered. Switch to an `sk_live_` key to
//! send for real.
//!
//! # Configuration
//!
//! ```no_run
//! # use std::time::Duration;
//! # fn run() -> Result<(), sendbyte::Error> {
//! use sendbyte::Client;
//!
//! let client = Client::builder("sk_live_...")
//!     .base_url("https://api.sendbyte.africa")
//!     .max_attempts(5)
//!     .timeout(Duration::from_secs(15))
//!     .build()?;
//! # let _ = client;
//! # Ok(())
//! # }
//! ```
//!
//! # Verifying webhook signatures
//!
//! Every webhook request carries a `sendbyte-signature` header. Verify it
//! against the raw request body with [`verify_webhook_signature`] before
//! trusting the payload.

mod client;
mod domains;
mod emails;
mod error;
mod signature;
mod webhooks;

pub use client::{Client, ClientBuilder};
pub use domains::{DnsRecord, Domain, DomainCheck, DomainVerification, Domains};
pub use emails::{
    Attachment, Email, EmailDetail, EmailEvent, EmailList, Emails, ListEmailsParams,
    ListUnsubscribe, SendEmailRequest,
};
pub use error::{Error, Result};
pub use signature::{
    verify_webhook_signature, verify_webhook_signature_with_tolerance, SIGNATURE_HEADER,
};
pub use webhooks::{WebhookEndpoint, WebhookEvents, WebhookReplay, Webhooks};
