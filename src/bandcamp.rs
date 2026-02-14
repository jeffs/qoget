use std::collections::HashMap;
use std::io::{Cursor, Read as _};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::Deserialize;

use crate::models::{
    Album, AlbumId, Artist, BandcampCollectionItem, BandcampCollectionResponse,
    BandcampDownloadInfo, DiscNumber, PurchaseList, Track, TrackId, TrackNumber,
};

const BASE_URL: &str = "https://bandcamp.com";
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36";
const ITEMS_PER_PAGE: u32 = 100;
const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF: Duration = Duration::from_secs(1);
const RATE_LIMIT_BACKOFF: Duration = Duration::from_secs(10);

// --- Rate limiter ---

struct RateLimiter {
    last_request: Mutex<Instant>,
    min_interval: Duration,
}

impl RateLimiter {
    fn new(requests_per_second: f64) -> Self {
        Self {
            last_request: Mutex::new(Instant::now() - Duration::from_secs(1)),
            min_interval: Duration::from_secs_f64(1.0 / requests_per_second),
        }
    }

    async fn wait(&self) {
        let wait_until = {
            let mut last = self.last_request.lock().unwrap();
            let now = Instant::now();
            let earliest = *last + self.min_interval;
            *last = earliest.max(now);
            earliest
        };
        let now = Instant::now();
        if wait_until > now {
            tokio::time::sleep(wait_until - now).await;
        }
    }
}

// --- Bandcamp client ---

pub struct BandcampClient {
    http: reqwest::Client,
    #[allow(dead_code)]
    identity_cookie: String,
    rate_limiter: RateLimiter,
}

/// Result of fetching all purchases: items + their redownload URLs.
pub struct BandcampPurchases {
    pub items: Vec<BandcampCollectionItem>,
    pub redownload_urls: HashMap<String, String>,
}

/// A single track extracted from a ZIP or downloaded directly.
pub struct ExtractedTrack {
    pub track_number: u8,
    pub title: String,
    pub temp_path: PathBuf,
}

// Helper for collection_summary response
#[derive(Deserialize)]
struct CollectionSummaryResponse {
    fan_id: u64,
}

impl BandcampClient {
    pub fn new(identity_cookie: String) -> Result<Self> {
        // Build cookie jar with identity cookie on bandcamp.com
        let jar = reqwest::cookie::Jar::default();
        let url = BASE_URL.parse::<reqwest::Url>().unwrap();
        jar.add_cookie_str(&format!("identity={}", identity_cookie), &url);

        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .cookie_provider(std::sync::Arc::new(jar))
            .build()
            .context("Failed to build HTTP client")?;

        Ok(Self {
            http,
            identity_cookie,
            rate_limiter: RateLimiter::new(3.0),
        })
    }

    /// Verify authentication and return the fan_id.
    pub async fn verify_auth(&self) -> Result<u64> {
        self.rate_limiter.wait().await;
        let resp = self
            .http
            .get(format!("{}/api/fan/2/collection_summary", BASE_URL))
            .send()
            .await
            .context("Failed to reach Bandcamp")?;

        let status = resp.status();
        if status == 401 || status == 403 {
            bail!("Bandcamp authentication failed: identity cookie is invalid or expired. \
                   Update BANDCAMP_IDENTITY or [bandcamp] identity_cookie in config.");
        }
        if !status.is_success() {
            bail!("Bandcamp collection_summary returned HTTP {}", status);
        }

        let summary: CollectionSummaryResponse = resp
            .json()
            .await
            .context("Failed to parse collection_summary response")?;
        Ok(summary.fan_id)
    }

    /// Fetch all purchases (collection items + hidden items) with pagination.
    pub async fn get_purchases(&self, fan_id: u64) -> Result<BandcampPurchases> {
        let mut all_items = Vec::new();
        let mut all_urls: HashMap<String, String> = HashMap::new();

        // Fetch visible collection items
        self.fetch_paginated_items(fan_id, "collection_items", &mut all_items, &mut all_urls)
            .await?;

        // Fetch hidden items
        self.fetch_paginated_items(fan_id, "hidden_items", &mut all_items, &mut all_urls)
            .await?;

        Ok(BandcampPurchases {
            items: all_items,
            redownload_urls: all_urls,
        })
    }

    async fn fetch_paginated_items(
        &self,
        fan_id: u64,
        endpoint: &str,
        items: &mut Vec<BandcampCollectionItem>,
        urls: &mut HashMap<String, String>,
    ) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        let mut older_than_token = format!("{now}:0:a::");

        loop {
            self.rate_limiter.wait().await;

            let body = serde_json::json!({
                "fan_id": fan_id.to_string(),
                "older_than_token": older_than_token,
                "count": ITEMS_PER_PAGE,
            });

            let resp: BandcampCollectionResponse = self
                .send_with_retry(
                    self.http
                        .post(format!("{}/api/fancollection/1/{}", BASE_URL, endpoint))
                        .json(&body),
                )
                .await
                .with_context(|| format!("Failed to fetch {endpoint}"))?;

            if resp.items.is_empty() {
                break;
            }

            // Grab the pagination token from the last item
            older_than_token = resp.items.last().unwrap().token.clone();

            urls.extend(resp.redownload_urls);
            items.extend(resp.items);

            if !resp.more_available {
                break;
            }
        }

        Ok(())
    }

    /// Get download info for a purchase by fetching the download page HTML.
    pub async fn get_download_info(&self, redownload_url: &str) -> Result<BandcampDownloadInfo> {
        self.rate_limiter.wait().await;

        let html = self
            .send_text_with_retry(self.http.get(redownload_url))
            .await
            .context("Failed to fetch download page")?;

        parse_download_page(&html)
    }

    /// Download an album ZIP (or single track file) and extract .m4a files.
    pub async fn download_and_extract(
        &self,
        download_url: &str,
        temp_dir: &Path,
    ) -> Result<Vec<ExtractedTrack>> {
        self.rate_limiter.wait().await;

        let resp = self
            .http
            .get(download_url)
            .send()
            .await
            .context("Failed to download file")?;

        if !resp.status().is_success() {
            bail!("Download returned HTTP {}", resp.status());
        }

        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let bytes = resp.bytes().await.context("Failed to read download body")?;

        if content_type.contains("zip") || is_zip_magic(&bytes) {
            extract_zip(&bytes, temp_dir)
        } else {
            // Single track — bare audio file
            extract_single_track(&bytes, temp_dir, download_url)
        }
    }

    /// Send a JSON request with retry on transient failures.
    async fn send_with_retry<T: serde::de::DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<T> {
        let mut backoff = INITIAL_BACKOFF;

        for attempt in 0..=MAX_RETRIES {
            self.rate_limiter.wait().await;

            let req = request
                .try_clone()
                .context("Request cannot be cloned for retry")?;

            let resp = req.send().await?;
            let status = resp.status();

            if status.is_success() {
                return resp.json().await.context("Failed to parse response JSON");
            }

            if status.as_u16() == 429 {
                if attempt < MAX_RETRIES {
                    eprintln!("HTTP 429 rate limited, backing off {:?}...", RATE_LIMIT_BACKOFF);
                    tokio::time::sleep(RATE_LIMIT_BACKOFF).await;
                    continue;
                }
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

    /// Send a request expecting text response, with retry.
    async fn send_text_with_retry(&self, request: reqwest::RequestBuilder) -> Result<String> {
        let mut backoff = INITIAL_BACKOFF;

        for attempt in 0..=MAX_RETRIES {
            self.rate_limiter.wait().await;

            let req = request
                .try_clone()
                .context("Request cannot be cloned for retry")?;

            let resp = req.send().await?;
            let status = resp.status();

            if status.is_success() {
                return resp.text().await.context("Failed to read response text");
            }

            if status.as_u16() == 429 {
                if attempt < MAX_RETRIES {
                    eprintln!("HTTP 429 rate limited, backing off {:?}...", RATE_LIMIT_BACKOFF);
                    tokio::time::sleep(RATE_LIMIT_BACKOFF).await;
                    continue;
                }
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
}

// --- HTML parsing ---

/// Parse the download page HTML to extract BandcampDownloadInfo.
/// Looks for `<div id="pagedata" data-blob="...">` and decodes the HTML entities.
fn parse_download_page(html: &str) -> Result<BandcampDownloadInfo> {
    let re = Regex::new(r#"id="pagedata"\s+data-blob="([^"]+)""#)?;
    let caps = re
        .captures(html)
        .context("Could not find pagedata data-blob in download page HTML")?;

    let encoded = &caps[1];
    let decoded = decode_html_entities(encoded);

    #[derive(Deserialize)]
    struct PageData {
        digital_items: Vec<BandcampDownloadInfo>,
    }

    let page_data: PageData =
        serde_json::from_str(&decoded).context("Failed to parse data-blob JSON")?;

    page_data
        .digital_items
        .into_iter()
        .next()
        .context("No digital_items found in download page")
}

/// Decode common HTML entities in a data-blob attribute value.
fn decode_html_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
}

/// Get the aac-hi download URL from a BandcampDownloadInfo, or error.
pub fn aac_hi_url(info: &BandcampDownloadInfo) -> Result<&str> {
    info.downloads
        .get("aac-hi")
        .map(|f| f.url.as_str())
        .context(format!(
            "No aac-hi format available for \"{}\" by {}. Available formats: {}",
            info.title,
            info.artist,
            info.downloads.keys().cloned().collect::<Vec<_>>().join(", ")
        ))
}

// --- ZIP extraction ---

fn is_zip_magic(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[..4] == [0x50, 0x4B, 0x03, 0x04]
}

/// Extract .m4a files from a ZIP archive. Returns extracted tracks with metadata.
fn extract_zip(zip_bytes: &[u8], temp_dir: &Path) -> Result<Vec<ExtractedTrack>> {
    let reader = Cursor::new(zip_bytes);
    let mut archive =
        zip::ZipArchive::new(reader).context("Failed to open ZIP archive")?;

    let mut tracks = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();

        // Skip directories and non-m4a files
        if entry.is_dir() || !name.to_lowercase().ends_with(".m4a") {
            continue;
        }

        // Extract filename from path (may include directory prefix like "Artist - Album/")
        let filename = Path::new(&name)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or(&name);

        let (track_number, title) = parse_zip_track_filename(filename);

        let temp_path = temp_dir.join(format!("bc_extract_{i}.m4a"));
        let mut buf = Vec::new();
        entry
            .read_to_end(&mut buf)
            .with_context(|| format!("Failed to read ZIP entry: {name}"))?;
        std::fs::write(&temp_path, &buf)
            .with_context(|| format!("Failed to write temp file: {}", temp_path.display()))?;

        tracks.push(ExtractedTrack {
            track_number,
            title,
            temp_path,
        });
    }

    // Sort by track number for consistent ordering
    tracks.sort_by_key(|t| t.track_number);

    Ok(tracks)
}

/// Extract a single track from a bare audio file response.
fn extract_single_track(
    bytes: &[u8],
    temp_dir: &Path,
    download_url: &str,
) -> Result<Vec<ExtractedTrack>> {
    let temp_path = temp_dir.join("bc_extract_single.m4a");
    std::fs::write(&temp_path, bytes)
        .with_context(|| format!("Failed to write temp file: {}", temp_path.display()))?;

    // Try to extract title from URL or content-disposition
    let title = extract_title_from_url(download_url);

    Ok(vec![ExtractedTrack {
        track_number: 1,
        title,
        temp_path,
    }])
}

fn extract_title_from_url(url: &str) -> String {
    // Best effort: grab the last path segment before query params
    url.split('?')
        .next()
        .and_then(|path| path.rsplit('/').next())
        .map(|s| s.trim_end_matches(".m4a").to_string())
        .unwrap_or_else(|| "Unknown".to_string())
}

/// Parse Bandcamp ZIP entry filenames: "NN TrackTitle.m4a" or "NN - TrackTitle.m4a"
pub fn parse_zip_track_filename(filename: &str) -> (u8, String) {
    let stem = filename.trim_end_matches(".m4a").trim_end_matches(".M4A");

    // Try to extract leading digits as track number
    let digits: String = stem.chars().take_while(|c| c.is_ascii_digit()).collect();

    if digits.is_empty() {
        return (0, stem.to_string());
    }

    let track_number = digits.parse::<u8>().unwrap_or(0);
    let rest = &stem[digits.len()..];

    // Strip separator: space, " - ", etc.
    let title = rest
        .trim_start_matches(" - ")
        .trim_start_matches(". ")
        .trim_start()
        .to_string();

    (track_number, title)
}

// --- Conversion to PurchaseList ---

/// Convert Bandcamp collection items to the shared PurchaseList format.
///
/// Groups items by sale_item_type: albums get full Album structs (tracks filled
/// later during download), individual tracks get standalone Album wrappers.
pub fn to_purchase_list(
    purchases: &BandcampPurchases,
) -> PurchaseList {
    let mut albums = Vec::new();
    let mut tracks = Vec::new();

    for item in &purchases.items {
        let artist = Artist {
            id: item.sale_item_id,
            name: item.band_name.clone(),
        };

        match item.sale_item_type.as_str() {
            "a" => {
                // Album purchase — tracks are populated during download (from ZIP contents)
                albums.push(Album {
                    id: AlbumId(format!("bc-{}", item.item_id)),
                    title: item.item_title.clone(),
                    version: None,
                    artist,
                    media_count: 1,
                    tracks_count: 0, // Unknown until we download
                    tracks: None,    // Populated during download
                });
            }
            "t" => {
                // Individual track purchase
                let track = Track {
                    id: TrackId(item.item_id),
                    title: item.item_title.clone(),
                    track_number: TrackNumber(1),
                    media_number: DiscNumber(1),
                    duration: 0,
                    performer: artist,
                    isrc: None,
                };
                tracks.push(track);
            }
            other => {
                eprintln!(
                    "Warning: unknown Bandcamp sale_item_type '{}' for '{}'",
                    other, item.item_title
                );
            }
        }
    }

    PurchaseList { albums, tracks }
}
