//! Cloudflare Turnstile CAPTCHA verification.

use serde::Deserialize;

#[derive(Deserialize)]
struct TurnstileResponse {
    success: bool,
}

/// Validates a Turnstile token against Cloudflare's API.
///
/// Returns `true` immediately if `secret` is empty — this skips validation
/// in local development where `TURNSTILE_SECRET` is not set.
pub async fn verify(client: &reqwest::Client, secret: &str, token: &str) -> bool {
    if secret.is_empty() {
        return true;
    }
    let resp = client
        .post("https://challenges.cloudflare.com/turnstile/v1/siteverify")
        .form(&[("secret", secret), ("response", token)])
        .send()
        .await;

    match resp {
        Ok(r) => r.json::<TurnstileResponse>().await.map(|r| r.success).unwrap_or(false),
        Err(_) => false,
    }
}
