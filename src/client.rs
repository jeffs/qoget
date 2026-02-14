use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::RequestBuilder;
use serde::de::DeserializeOwned;

use crate::models::{
    Album, AlbumId, FileUrlResponse, LoginResponse, PurchaseList, PurchaseResponse, TrackId,
    UserAuth,
};

const BASE_URL: &str = "https://www.qobuz.com/api.json/0.2";
const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF: Duration = Duration::from_secs(1);

pub struct QobuzClient {
    http: reqwest::Client,
    app_id: String,
    app_secret: String,
    auth_token: String,
}

impl QobuzClient {
    pub fn new(
        http: reqwest::Client,
        app_id: String,
        app_secret: String,
        auth_token: String,
    ) -> Self {
        Self { http, app_id, app_secret, auth_token }
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    fn authed_get(&self, path: &str) -> RequestBuilder {
        self.http
            .get(format!("{}{}", BASE_URL, path))
            .header("X-App-Id", &self.app_id)
            .header("X-User-Auth-Token", &self.auth_token)
    }

    /// Fetch all purchases, paginating through albums and tracks.
    pub async fn get_purchases(&self) -> Result<PurchaseList> {
        let mut all_albums = Vec::new();
        let mut all_tracks = Vec::new();
        let limit: u64 = 500;

        let mut offset: u64 = 0;
        loop {
            let resp: PurchaseResponse = send_with_retry(
                self.authed_get("/purchase/getUserPurchases")
                    .query(&[
                        ("limit", limit.to_string()),
                        ("offset", offset.to_string()),
                    ]),
            )
            .await
            .context("Failed to fetch purchases")?;

            all_albums.extend(resp.albums.items);
            all_tracks.extend(resp.tracks.items);

            if offset + limit >= resp.albums.total {
                break;
            }
            offset += limit;
        }

        Ok(PurchaseList {
            albums: all_albums,
            tracks: all_tracks,
        })
    }

    /// Fetch full album metadata including track listing.
    pub async fn get_album(&self, album_id: &AlbumId) -> Result<Album> {
        send_with_retry(
            self.authed_get("/album/get")
                .query(&[("album_id", album_id.0.as_str())]),
        )
        .await
        .context("Failed to fetch album")
    }

    /// Get a signed download URL for a track.
    ///
    /// Uses `intent=stream` in both the query and signature. Qobuz now validates
    /// the intent parameter against the signature (previously it was ignored
    /// server-side). Using `intent=stream` with `format_id=5` still returns
    /// MP3 320 URLs for purchased content.
    pub async fn get_file_url(
        &self,
        track_id: TrackId,
        format_id: u8,
    ) -> Result<String> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
            .to_string();

        let sig = generate_request_sig(track_id.0, format_id, &timestamp, &self.app_secret);

        let resp: FileUrlResponse = send_with_retry(
            self.authed_get("/track/getFileUrl")
                .query(&[
                    ("track_id", track_id.0.to_string()),
                    ("format_id", format_id.to_string()),
                    ("intent", "stream".to_string()),
                    ("request_ts", timestamp),
                    ("request_sig", sig),
                ]),
        )
        .await
        .context("Failed to get file URL")?;

        Ok(resp.url)
    }
}

/// Authenticate with Qobuz. Returns auth token and user ID.
pub async fn login(
    http: &reqwest::Client,
    app_id: &str,
    username: &str,
    password: &str,
) -> Result<UserAuth> {
    let password_hash = format!("{:x}", md5::compute(password.as_bytes()));

    let resp = http
        .get(format!("{}/user/login", BASE_URL))
        .header("X-App-Id", app_id)
        .query(&[
            ("email", username),
            ("password", &password_hash),
            ("app_id", app_id),
        ])
        .send()
        .await
        .context("Login request failed")?;

    if resp.status() == 401 {
        bail!("Authentication failed: invalid credentials");
    }

    let login: LoginResponse = resp
        .json()
        .await
        .context("Failed to parse login response")?;

    Ok(UserAuth {
        token: login.user_auth_token,
        user_id: login.user.id,
    })
}

/// Generate the MD5 request signature for /track/getFileUrl.
/// Signature always uses "intentstream" regardless of actual intent parameter.
pub fn generate_request_sig(
    track_id: u64,
    format_id: u8,
    timestamp: &str,
    app_secret: &str,
) -> String {
    let data = format!(
        "trackgetFileUrlformat_id{format_id}intentstreamtrack_id{track_id}{timestamp}{app_secret}"
    );
    format!("{:x}", md5::compute(data.as_bytes()))
}

/// Send a request with retry on transient failures (429, 500, 502, 503, 504).
/// Exponential backoff: 1s, 2s, 4s. Max 3 retries.
/// Does NOT retry on 401 (auth) or 400 (bad request).
async fn send_with_retry<T: DeserializeOwned>(request: RequestBuilder) -> Result<T> {
    let mut backoff = INITIAL_BACKOFF;

    for attempt in 0..=MAX_RETRIES {
        let req = request
            .try_clone()
            .context("Request cannot be cloned for retry")?;

        let resp = req.send().await?;
        let status = resp.status();

        if status.is_success() {
            return resp.json().await.context("Failed to parse response JSON");
        }

        let retryable = matches!(status.as_u16(), 429 | 500 | 502 | 503 | 504);

        if !retryable || attempt == MAX_RETRIES {
            let body = resp.text().await.unwrap_or_default();
            bail!("HTTP {} — {}", status, body);
        }

        eprintln!("HTTP {}, retrying in {:?}...", status, backoff);
        tokio::time::sleep(backoff).await;
        backoff *= 2;
    }

    unreachable!()
}
