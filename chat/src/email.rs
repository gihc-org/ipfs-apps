//! Email delivery via the Resend API.

use serde::Serialize;

#[derive(Serialize)]
struct SendEmail<'a> {
    from: &'a str,
    to: [&'a str; 1],
    subject: &'a str,
    html: String,
}

/// Sends a verification email with a link the user must click to activate their account.
///
/// If `api_key` is empty (local development), the verification URL is printed to
/// the log instead of sent, and the function returns `Ok(())` so registration
/// still succeeds without email infrastructure.
pub async fn send_verification(
    client: &reqwest::Client,
    api_key: &str,
    from: &str,
    to: &str,
    verify_url: &str,
) -> Result<(), String> {
    if api_key.is_empty() {
        tracing::info!(verify_url, "RESEND_API_KEY not set — skipping email, verify URL logged");
        return Ok(());
    }

    let body = SendEmail {
        from,
        to: [to],
        subject: "Bekræft din email — Chat",
        html: format!(
            "<p>Hej,</p>\
             <p>Klik på linket herunder for at bekræfte din email-adresse:</p>\
             <p><a href=\"{url}\">{url}</a></p>\
             <p>Linket udløber ikke — men du kan ikke logge ind før du har bekræftet.</p>",
            url = verify_url
        ),
    };

    let resp = client
        .post("https://api.resend.com/emails")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("Resend returned {}", resp.status()))
    }
}
