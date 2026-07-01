//! Run with: SENDBYTE_API_KEY=sk_test_... cargo run --example send_email

use sendbyte::{Client, SendEmailRequest};

#[tokio::main]
async fn main() -> Result<(), sendbyte::Error> {
    let api_key = std::env::var("SENDBYTE_API_KEY")
        .expect("set SENDBYTE_API_KEY to an sk_test_ or sk_live_ key");

    let client = Client::new(api_key)?;

    let email = client
        .emails()
        .send(
            SendEmailRequest::new("PayLink <receipts@paylink.ng>", ["amaka@halo.ng"])
                .subject("Receipt for your payment")
                .html("<p>Thank you. Your payment was received.</p>")
                .tags(["receipt"]),
        )
        .await?;

    println!("queued email {} with status {}", email.id, email.status);
    Ok(())
}
