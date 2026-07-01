//! Throwaway smoke test against the live sandbox.
//! Run: SENDBYTE_API_KEY=sk_test_... cargo run --example smoke

use sendbyte::{Client, ListEmailsParams, SendEmailRequest};

#[tokio::main]
async fn main() {
    let key = std::env::var("SENDBYTE_API_KEY").expect("set SENDBYTE_API_KEY");
    let client = Client::new(key).expect("client builds");

    println!("== send ==");
    match client
        .emails()
        .send(
            SendEmailRequest::new("SendByte Test <test@sendbyte.africa>", ["dev@example.com"])
                .subject("Rust SDK smoke test")
                .html("<p>Hello from sendbyte-rust.</p>")
                .tags(["smoke"]),
        )
        .await
    {
        Ok(email) => {
            println!("  ok: id={} status={} sandbox={}", email.id, email.status, email.sandbox);
            println!("== get ==");
            match client.emails().get(&email.id).await {
                Ok(detail) => println!(
                    "  ok: id={} events={}",
                    detail.email.id,
                    detail.events.len()
                ),
                Err(e) => println!("  err: {e} (code={}, status={:?})", e.code, e.status),
            }
        }
        Err(e) => println!("  err: {e} (code={}, status={:?})", e.code, e.status),
    }

    println!("== list emails ==");
    match client.emails().list(&ListEmailsParams::new().limit(3)).await {
        Ok(page) => println!("  ok: {} emails, has_more={}", page.data.len(), page.has_more),
        Err(e) => println!("  err: {e} (code={}, status={:?})", e.code, e.status),
    }

    println!("== list domains ==");
    match client.domains().list().await {
        Ok(domains) => println!("  ok: {} domains", domains.len()),
        Err(e) => println!("  err: {e} (code={}, status={:?})", e.code, e.status),
    }

    println!("== list webhooks ==");
    match client.webhooks().list().await {
        Ok(hooks) => println!("  ok: {} webhooks", hooks.len()),
        Err(e) => println!("  err: {e} (code={}, status={:?})", e.code, e.status),
    }
}
