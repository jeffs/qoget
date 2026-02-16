use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Deserializer};

fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(|opt| opt.unwrap_or_default())
}

// --- Service enum ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Service {
    Qobuz,
    Bandcamp,
}

impl fmt::Display for Service {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Service::Qobuz => write!(f, "Qobuz"),
            Service::Bandcamp => write!(f, "Bandcamp"),
        }
    }
}

// --- Newtype wrappers ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(transparent)]
pub struct TrackId(pub u64);

impl fmt::Display for TrackId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(transparent)]
pub struct AlbumId(pub String);

impl fmt::Display for AlbumId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct TrackNumber(pub u8);

impl fmt::Display for TrackNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct DiscNumber(pub u8);

impl fmt::Display for DiscNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// --- API response types (serde) ---

#[derive(Debug, Clone, Deserialize)]
pub struct Artist {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Album {
    pub id: AlbumId,
    pub title: String,
    pub version: Option<String>,
    pub artist: Artist,
    pub media_count: u8,
    pub tracks_count: u16,
    #[serde(default)]
    pub tracks: Option<PaginatedList<Track>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub title: String,
    pub track_number: TrackNumber,
    pub media_number: DiscNumber,
    pub duration: u32,
    pub performer: Artist,
    pub isrc: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PaginatedList<T> {
    pub offset: u64,
    pub limit: u64,
    pub total: u64,
    pub items: Vec<T>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PurchaseResponse {
    pub albums: PaginatedList<Album>,
    pub tracks: PaginatedList<Track>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginResponse {
    pub user_auth_token: String,
    pub user: UserInfo,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserInfo {
    pub id: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileUrlResponse {
    pub track_id: u64,
    pub url: String,
    pub format_id: u8,
    pub mime_type: String,
}

// --- Domain types ---

pub struct AppCredentials {
    pub app_id: String,
    pub app_secret: String,
}

pub struct UserAuth {
    pub token: String,
    pub user_id: u64,
}

pub struct Session {
    pub credentials: AppCredentials,
    pub auth: UserAuth,
}

/// All purchases aggregated across paginated responses.
pub struct PurchaseList {
    pub albums: Vec<Album>,
    pub tracks: Vec<Track>,
}

pub struct DownloadTask {
    pub track: Track,
    pub album: Album,
    pub target_path: PathBuf,
    pub file_extension: &'static str,
}

pub enum SkipReason {
    AlreadyExists,
    DryRun,
}

pub struct SkippedTrack {
    pub track: Track,
    pub target_path: PathBuf,
    pub reason: SkipReason,
}

pub struct SyncPlan {
    pub downloads: Vec<DownloadTask>,
    pub skipped: Vec<SkippedTrack>,
    pub total_tracks: usize,
}

pub struct DownloadError {
    pub task: DownloadTask,
    pub error: String,
}

pub struct SyncResult {
    pub succeeded: Vec<DownloadTask>,
    pub failed: Vec<DownloadError>,
    pub skipped: Vec<SkippedTrack>,
    pub fallback_count: usize,
}

// --- Bandcamp API response types ---

#[derive(Debug, Clone, Deserialize)]
pub struct BandcampCollectionResponse {
    pub more_available: bool,
    #[serde(deserialize_with = "null_as_default")]
    pub last_token: String,
    #[serde(deserialize_with = "null_as_default")]
    pub redownload_urls: HashMap<String, String>,
    pub items: Vec<BandcampCollectionItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BandcampCollectionItem {
    #[serde(deserialize_with = "null_as_default")]
    pub band_name: String,
    #[serde(deserialize_with = "null_as_default")]
    pub item_title: String,
    pub item_id: u64,
    #[serde(deserialize_with = "null_as_default")]
    pub item_type: String,
    #[serde(deserialize_with = "null_as_default")]
    pub sale_item_type: String,
    pub sale_item_id: u64,
    #[serde(deserialize_with = "null_as_default")]
    pub token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BandcampDownloadInfo {
    pub item_id: u64,
    pub title: String,
    pub artist: String,
    pub download_type: String,
    pub downloads: HashMap<String, BandcampDownloadFormat>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BandcampDownloadFormat {
    pub url: String,
    pub size_mb: String,
}

// --- Bandcamp sync result ---

pub struct BandcampSyncResult {
    pub downloaded: usize,
    pub skipped: usize,
    pub would_download: usize,
    pub failed: Vec<BandcampDownloadError>,
}

pub struct BandcampDownloadError {
    pub description: String,
    pub error: String,
}
