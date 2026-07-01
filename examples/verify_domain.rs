//! Run with: SENDBYTE_API_KEY=sk_test_... cargo run --example verify_domain -- paylink.ng

use sendbyte::Client;

#[tokio::main]
async fn main() -> Result<(), sendbyte::Error> {
    let api_key = std::env::var("SENDBYTE_API_KEY")
        .expect("set SENDBYTE_API_KEY to an sk_test_ or sk_live_ key");
    let domain = std::env::args()
        .nth(1)
        .expect("pass a domain, e.g. paylink.ng");

    let client = Client::new(api_key)?;

    let created = client.domains().create(&domain).await?;
    println!("registered {} ({})", created.domain, created.id);
    println!("publish these DNS records:");
    for record in &created.dns_records {
        println!(
            "  {:<5} {:<24} {}  [{}]",
            record.record_type, record.name, record.value, record.purpose
        );
    }

    let check = client.domains().verify(&created.id).await?;
    println!(
        "status: {} (deliverability_ready: {})",
        check.status, check.deliverability_ready
    );
    Ok(())
}
