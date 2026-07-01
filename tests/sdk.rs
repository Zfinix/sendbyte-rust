use sendbyte::{Client, ListEmailsParams, SendEmailRequest, WebhookEvents};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn client_for(server: &MockServer) -> Client {
    Client::builder("sk_test_123")
        .base_url(server.uri())
        .build()
        .expect("client builds")
}

#[tokio::test]
async fn send_email_posts_payload_and_injects_idempotency_key() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/emails"))
        .and(header("authorization", "Bearer sk_test_123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "em_1",
            "from": "a@b.ng",
            "to": ["c@d.ng"],
            "subject": "Hi",
            "status": "queued",
            "sandbox": true,
            "tags": [],
            "message_id": null,
            "scheduled_at": null,
            "created_at": "2026-07-01T00:00:00Z"
        })))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let email = client
        .emails()
        .send(
            SendEmailRequest::new("a@b.ng", ["c@d.ng"])
                .subject("Hi")
                .html("<p>Hi</p>"),
        )
        .await
        .unwrap();

    assert_eq!(email.id, "em_1");
    assert_eq!(email.status, "queued");

    let received = &server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&received.last().unwrap().body).unwrap();
    assert!(
        body.get("idempotency_key").is_some(),
        "send() should inject an idempotency_key"
    );
}

#[tokio::test]
async fn list_emails_builds_query_string() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/emails"))
        .and(query_param("limit", "20"))
        .and(query_param("status", "delivered"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [],
            "has_more": false
        })))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let page = client
        .emails()
        .list(&ListEmailsParams::new().limit(20).status("delivered"))
        .await
        .unwrap();
    assert!(!page.has_more);
}

#[tokio::test]
async fn retries_on_500_then_succeeds() {
    let server = MockServer::start().await;
    // First: a 500 (retryable).
    Mock::given(method("GET"))
        .and(path("/v1/domains/dm_1"))
        .respond_with(ResponseTemplate::new(500).set_body_string("upstream boom"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    // Then: success.
    Mock::given(method("GET"))
        .and(path("/v1/domains/dm_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "dm_1",
            "domain": "paylink.ng",
            "status": "verified",
            "dkim_selector": "sb1",
            "dns_records": [],
            "verified_at": "2026-07-01T00:00:00Z",
            "deliverability_ready": true,
            "created_at": "2026-06-01T00:00:00Z"
        })))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let domain = client.domains().get("dm_1").await.unwrap();
    assert_eq!(domain.domain, "paylink.ng");
    assert!(domain.deliverability_ready);
}

#[tokio::test]
async fn typed_error_on_4xx() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/domains"))
        .respond_with(ResponseTemplate::new(422).set_body_json(serde_json::json!({
            "error": {
                "code": "domain_taken",
                "message": "That domain is already registered.",
                "docs_url": "https://docs.sendbyte.africa/errors/domain_taken"
            }
        })))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let err = client.domains().create("paylink.ng").await.unwrap_err();
    assert_eq!(err.code, "domain_taken");
    assert_eq!(err.status, Some(422));
    assert!(err.docs_url.is_some());
    assert!(!err.is_retryable());
}

#[tokio::test]
async fn webhook_events_all_and_named() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/webhooks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [
                {
                    "id": "wh_all",
                    "url": "https://a.ng/hooks",
                    "events": "all",
                    "disabled": false,
                    "disabled_at": null,
                    "consecutive_failures": 0,
                    "created_at": "2026-07-01T00:00:00Z"
                },
                {
                    "id": "wh_some",
                    "url": "https://b.ng/hooks",
                    "events": ["email.delivered", "email.bounced"],
                    "disabled": false,
                    "disabled_at": null,
                    "consecutive_failures": 2,
                    "created_at": "2026-07-01T00:00:00Z"
                }
            ]
        })))
        .mount(&server)
        .await;

    let client = client_for(&server).await;
    let endpoints = client.webhooks().list().await.unwrap();
    assert_eq!(endpoints[0].events, WebhookEvents::All);
    assert!(endpoints[0].events.is_all());
    assert_eq!(
        endpoints[1].events.names(),
        ["email.delivered", "email.bounced"]
    );
    assert_eq!(endpoints[1].consecutive_failures, 2);
}
