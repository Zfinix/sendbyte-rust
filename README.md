# sendbyte-rust

An unofficial Rust SDK for [SendByte](https://sendbyte.africa), the email API for Africa. Async, built on `reqwest` and `tokio`.

> Community-maintained. Not affiliated with or endorsed by SendByte.

## Install

```bash
cargo add sendbyte-sdk
```

The crate is published as `sendbyte-sdk`; the library is imported as `sendbyte` (`use sendbyte::...`). Requires Rust 1.75 or newer, and an async runtime (`tokio`).

## Quickstart

```rust
use sendbyte::{Client, SendEmailRequest};

#[tokio::main]
async fn main() -> Result<(), sendbyte::Error> {
    let client = Client::new("sk_test_...")?;

    let email = client
        .emails()
        .send(
            SendEmailRequest::new("PayLink <receipts@paylink.ng>", ["amaka@halo.ng"])
                .subject("Receipt for your payment")
                .html("<p>Thank you. Your payment was received.</p>"),
        )
        .await?;

    println!("{}", email.id);
    Ok(())
}
```

Keys that start with `sk_test_` run in the sandbox: every endpoint behaves the same, but nothing is actually delivered. Switch to an `sk_live_` key to send for real.

## Configuration

```rust
use std::time::Duration;
use sendbyte::Client;

let client = Client::builder("sk_live_...")
    .base_url("https://api.sendbyte.africa")
    .max_attempts(5)
    .timeout(Duration::from_secs(15))
    .build()?;
```

Requests retry on `429` and `5xx` responses (and transport errors) with exponential backoff plus jitter, honoring `Retry-After`, up to the configured attempts. On `POST /v1/emails` an idempotency key is generated and reused across retries when you do not supply one, so a retried send cannot double-send.

## API

### Emails

```rust
client.emails().send(SendEmailRequest::new(from, [to]).subject("...").html("...")).await?;
client.emails().get("em_123").await?;
client.emails().list(&ListEmailsParams::new().limit(20).status("delivered")).await?;
```

Use `.scheduled_at(...)` for future sends, and `.template(id, variables)` to render a server-side template. Pass `.idempotency_key(...)` to set your own key.

### Domains

```rust
let domain = client.domains().create("paylink.ng").await?;
// domain.dns_records lists the SPF, DKIM, and DMARC records to publish
client.domains().list().await?;
client.domains().get(&domain.id).await?;
client.domains().verify(&domain.id).await?;
```

### Webhooks

```rust
let endpoint = client
    .webhooks()
    .create("https://example.com/hooks/sendbyte", Some(vec![
        "email.delivered".into(),
        "email.bounced".into(),
    ]))
    .await?;
// endpoint.secret is shown once, store it now
client.webhooks().list().await?;
client.webhooks().disable(&endpoint.id).await?;
client.webhooks().replay("evt_123").await?;
```

Pass `None` for the events to subscribe to everything. An endpoint subscribed to every event reports `endpoint.events == WebhookEvents::All`; otherwise `endpoint.events.names()` holds the specific event names.

## Verifying webhook signatures

Every webhook request carries a `sendbyte-signature` header. Verify it against the raw request body before trusting the payload.

```rust
use sendbyte::{verify_webhook_signature, SIGNATURE_HEADER};

let signature = headers.get(SIGNATURE_HEADER).and_then(|v| v.to_str().ok());
if !verify_webhook_signature(secret, signature, &raw_body) {
    // reject with 401
}
```

The check rejects missing, malformed, tampered, and stale signatures (tolerance is 300 seconds; override with `verify_webhook_signature_with_tolerance`).

## Errors

Failed requests return a `sendbyte::Error` carrying the API error shape.

```rust
match client.emails().send(req).await {
    Ok(email) => println!("{}", email.id),
    Err(err) => eprintln!("{} {:?} {}", err.code, err.status, err.message),
}
```

## Links

- Documentation: https://docs.sendbyte.africa

## License

MIT
