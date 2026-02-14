use anyhow::{Context, Result, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use regex::Regex;

use crate::models::AppCredentials;

const LOGIN_URL: &str = "https://play.qobuz.com/login";
const PLAY_BASE: &str = "https://play.qobuz.com";
const VALIDATION_TRACK_ID: u64 = 19512574;
const VALIDATION_FORMAT_ID: u8 = 27;

/// Extract app_id and app_secret from the Qobuz web player's bundle.js.
pub async fn extract_credentials(http_client: &reqwest::Client) -> Result<AppCredentials> {
    // Step 1: Fetch login page and find bundle.js URL
    let login_html = http_client
        .get(LOGIN_URL)
        .send()
        .await
        .context("Failed to fetch Qobuz login page")?
        .text()
        .await?;

    let bundle_re = Regex::new(r#"<script src="(/resources/[^"]+/bundle\.js)">"#)?;
    let bundle_path = bundle_re
        .captures(&login_html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
        .context("Could not find bundle.js URL in login page")?;

    let bundle_url = format!("{}{}", PLAY_BASE, bundle_path);

    // Step 2: Fetch the bundle
    let bundle = http_client
        .get(&bundle_url)
        .send()
        .await
        .context("Failed to fetch bundle.js")?
        .text()
        .await?;

    // Step 3: Extract app_id
    let app_id_re = Regex::new(r#"production:\{api:\{appId:"(\d{9})""#)?;
    let app_id = app_id_re
        .captures(&bundle)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .context("Could not extract app_id from bundle.js")?;

    // Step 4: Extract seed/timezone pairs
    let seed_re = Regex::new(r#"[a-z]\.initialSeed\("([\w=]+)",window\.utimezone\.([a-z]+)\)"#)?;
    let mut seed_pairs: Vec<(String, String)> = seed_re
        .captures_iter(&bundle)
        .map(|c| (c[1].to_string(), c[2].to_string()))
        .collect();

    if seed_pairs.len() < 2 {
        bail!(
            "Expected at least 2 seed/timezone pairs, found {}",
            seed_pairs.len()
        );
    }

    // Step 5: Swap the first two pairs (ternary condition always evaluates to false)
    seed_pairs.swap(0, 1);

    // Step 6: For each timezone, find info/extras
    let mut candidate_secrets = Vec::new();
    for (seed, timezone) in &seed_pairs {
        let tz_capitalized = capitalize_first(timezone);
        let info_pattern = format!(
            r#"name:"\w+/{}",info:"([\w=]+)",extras:"([\w=]+)""#,
            regex::escape(&tz_capitalized)
        );
        let info_re = Regex::new(&info_pattern)?;

        if let Some(caps) = info_re.captures(&bundle) {
            let info = &caps[1];
            let extras = &caps[2];

            // Concatenate seed + info + extras, strip last 44 chars, base64-decode
            let combined = format!("{}{}{}", seed, info, extras);
            if combined.len() > 44 {
                let trimmed = &combined[..combined.len() - 44];
                match BASE64.decode(trimmed) {
                    Ok(decoded) => {
                        if let Ok(secret) = String::from_utf8(decoded) {
                            candidate_secrets.push(secret);
                        }
                    }
                    Err(_) => continue,
                }
            }
        }
    }

    if candidate_secrets.is_empty() {
        bail!("No candidate secrets could be extracted from bundle.js");
    }

    // Step 7: Validate each candidate secret
    for secret in &candidate_secrets {
        match validate_secret(http_client, &app_id, secret).await {
            Ok(true) => {
                return Ok(AppCredentials {
                    app_id,
                    app_secret: secret.clone(),
                });
            }
            Ok(false) => continue,
            Err(_) => continue,
        }
    }

    bail!(
        "No valid app_secret found among {} candidates",
        candidate_secrets.len()
    )
}

/// Validate a candidate secret by making a test request to /track/getFileUrl.
/// Returns Ok(true) if valid (HTTP 200 or 401), Ok(false) if invalid (HTTP 400).
async fn validate_secret(
    http_client: &reqwest::Client,
    app_id: &str,
    secret: &str,
) -> Result<bool> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs()
        .to_string();

    let sig = crate::client::generate_request_sig(
        VALIDATION_TRACK_ID,
        VALIDATION_FORMAT_ID,
        &timestamp,
        secret,
    );

    let resp = http_client
        .get("https://www.qobuz.com/api.json/0.2/track/getFileUrl")
        .header("X-App-Id", app_id)
        .query(&[
            ("track_id", VALIDATION_TRACK_ID.to_string()),
            ("format_id", VALIDATION_FORMAT_ID.to_string()),
            ("intent", "stream".to_string()),
            ("request_ts", timestamp),
            ("request_sig", sig),
        ])
        .send()
        .await?;

    match resp.status().as_u16() {
        200 | 401 => Ok(true),
        400 => Ok(false),
        other => bail!("Unexpected status {} during secret validation", other),
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let upper: String = first.to_uppercase().collect();
            format!("{}{}", upper, chars.as_str())
        }
    }
}
