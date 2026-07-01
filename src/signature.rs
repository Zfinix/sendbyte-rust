use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use sha2::Sha256;

/// The header carrying the webhook HMAC signature.
pub const SIGNATURE_HEADER: &str = "sendbyte-signature";

const DEFAULT_TOLERANCE_SECONDS: i64 = 300;

type HmacSha256 = Hmac<Sha256>;

/// Verifies that a webhook request came from SendByte.
///
/// Verify against the **raw** request body, before any JSON parsing. The header
/// format is `t=<unix seconds>,v1=<hex hmac-sha256>`, computed over
/// `<t>.<raw body>`. The check rejects missing, malformed, tampered, and stale
/// signatures (default tolerance 300 seconds).
///
/// ```
/// use sendbyte::verify_webhook_signature;
/// let ok = verify_webhook_signature("whsec_secret", Some("t=1,v1=deadbeef"), b"{}");
/// assert!(!ok); // stale + wrong signature
/// ```
pub fn verify_webhook_signature(secret: &str, header: Option<&str>, body: &[u8]) -> bool {
    verify_webhook_signature_with_tolerance(secret, header, body, DEFAULT_TOLERANCE_SECONDS)
}

/// Like [`verify_webhook_signature`] but with a caller-supplied tolerance (in
/// seconds) for the signature timestamp.
pub fn verify_webhook_signature_with_tolerance(
    secret: &str,
    header: Option<&str>,
    body: &[u8],
    tolerance_seconds: i64,
) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    verify_at(secret, header, body, tolerance_seconds, now)
}

fn verify_at(secret: &str, header: Option<&str>, body: &[u8], tolerance: i64, now: i64) -> bool {
    let Some(header) = header else {
        return false;
    };

    let mut timestamp: Option<i64> = None;
    let mut signature: Option<&str> = None;
    for part in header.split(',') {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        match key {
            "t" => timestamp = value.trim().parse().ok(),
            "v1" => signature = Some(value.trim()),
            _ => {}
        }
    }

    let (Some(timestamp), Some(signature)) = (timestamp, signature) else {
        return false;
    };
    if (now - timestamp).abs() > tolerance {
        return false;
    }
    let Ok(signature) = hex::decode(signature) else {
        return false;
    };

    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(mac) => mac,
        Err(_) => return false,
    };
    mac.update(format!("{timestamp}.").as_bytes());
    mac.update(body);
    // `verify_slice` is constant-time.
    mac.verify_slice(&signature).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sign(secret: &str, timestamp: i64, body: &[u8]) -> String {
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(format!("{timestamp}.").as_bytes());
        mac.update(body);
        let sig = hex::encode(mac.finalize().into_bytes());
        format!("t={timestamp},v1={sig}")
    }

    #[test]
    fn accepts_a_valid_signature() {
        let secret = "whsec_test";
        let body = br#"{"type":"email.delivered"}"#;
        let header = sign(secret, 1000, body);
        assert!(verify_at(secret, Some(&header), body, 300, 1000));
    }

    #[test]
    fn rejects_tampered_body() {
        let secret = "whsec_test";
        let header = sign(secret, 1000, b"original");
        assert!(!verify_at(secret, Some(&header), b"tampered", 300, 1000));
    }

    #[test]
    fn rejects_stale_and_missing_and_malformed() {
        let secret = "whsec_test";
        let body = b"payload";
        let header = sign(secret, 1000, body);
        assert!(!verify_at(secret, Some(&header), body, 300, 2000));
        assert!(!verify_at(secret, None, body, 300, 1000));
        assert!(!verify_at(secret, Some("garbage"), body, 300, 1000));
        assert!(!verify_at(secret, Some("t=1000"), body, 300, 1000));
    }
}
