# Data Model: Bandcamp Support & Unified CLI

**Branch**: `002-bandcamp-unified-cli` | **Date**: 2026-02-14

## Entities

### Service (new)

Represents a music purchase platform. Closed enum — only known services.

```
Service
├── Qobuz
└── Bandcamp
```

Used for: config loading, CLI filtering (`--service`), progress display, error reporting.

### ServiceConfig (new)

Per-service credential configuration. Sum type — each variant carries service-specific fields.

```
ServiceConfig
├── Qobuz { username, password, app_id?, app_secret? }
└── Bandcamp { identity_cookie }
```

**Validation rules**:
- Qobuz: `username` and `password` must be non-empty strings
- Bandcamp: `identity_cookie` must be non-empty string
- At least one service must be configured (or error at startup)

### Config (modified)

Top-level configuration aggregating all service configs.

```
Config {
  services: Vec<ServiceConfig>  // 1..N configured services
}
```

**Identity**: Uniqueness determined by service type — at most one config per service.

**Backward compatibility**: Bare `username`/`password` keys in TOML are desugared into a `Qobuz` service config during loading.

### Album (existing, unchanged)

```
Album {
  id: AlbumId(String)
  title: String
  version: Option<String>
  artist: Artist
  media_count: u8
  tracks_count: u16
  tracks: Option<PaginatedList<Track>>
}
```

### Track (existing, unchanged)

```
Track {
  id: TrackId(u64)
  title: String
  track_number: TrackNumber(u8)
  media_number: DiscNumber(u8)
  duration: u32
  performer: Artist
  isrc: Option<String>
}
```

### BandcampCollectionItem (new)

Raw item from Bandcamp's `collection_items` API. Intermediate type — converted to Album/Track before entering the sync pipeline.

```
BandcampCollectionItem {
  band_name: String
  item_title: String
  item_id: u64
  item_type: String       // "album" | "track"
  sale_item_type: String   // "a" | "t"
  sale_item_id: u64
  token: String            // pagination cursor
}
```

**Lifecycle**: Fetched → mapped to Album/Track → discarded.

### BandcampDownloadInfo (new)

Parsed from the download page's `digital_items` JSON. Contains format-specific download URLs.

```
BandcampDownloadInfo {
  item_id: u64
  title: String
  artist: String
  download_type: String    // "a" | "t"
  downloads: Map<String, DownloadFormat>
}

DownloadFormat {
  url: String
  size_mb: String
}
```

**Lifecycle**: Fetched per album/track → URL extracted → discarded.

### DownloadTask (existing, extended)

```
DownloadTask {
  track: Track
  album: Album
  target_path: PathBuf
  file_extension: String   // NEW: ".mp3" or ".m4a"
}
```

**New field**: `file_extension` — determined by the originating service. Qobuz = `.mp3`, Bandcamp = `.m4a`.

### SyncPlan (existing, unchanged)

```
SyncPlan {
  downloads: Vec<DownloadTask>
  skipped: Vec<SkippedTrack>
  total_tracks: usize
}
```

### SyncResult (existing, unchanged)

```
SyncResult {
  succeeded: Vec<DownloadTask>
  failed: Vec<DownloadError>
  skipped: Vec<SkippedTrack>
}
```

## Relationships

```
Config 1──* ServiceConfig
ServiceConfig ──> Service (enum variant)

Service ──produces──> PurchaseList { albums, tracks }

PurchaseList ──> Album 1──* Track

DownloadTask ──> Track
DownloadTask ──> Album
DownloadTask ──> PathBuf (target_path)

SyncPlan ──> DownloadTask (to download)
SyncPlan ──> SkippedTrack (already exists / dry-run)

SyncResult ──> DownloadTask (succeeded)
SyncResult ──> DownloadError (failed)
SyncResult ──> SkippedTrack (skipped)
```

## State Transitions

### Bandcamp Download Flow

```
CollectionItem
  │ (fetch collection_items API)
  ▼
RedownloadUrl
  │ (GET download page)
  ▼
DownloadPageBlob
  │ (parse digital_items JSON)
  ▼
DownloadInfo { downloads: { "aac-hi": { url } } }
  │ (select format, GET url)
  ▼
ZipFile (for albums) or AudioFile (for tracks)
  │ (extract if ZIP)
  ▼
Individual .m4a files on disk
```

### Unified Sync Flow

```
Config
  │ (load, determine configured services)
  ▼
[QobuzClient, BandcampClient]  (filtered by --service if specified)
  │ (fetch purchases from each)
  ▼
PurchaseList per service
  │ (collect_tasks per service, merge)
  ▼
Vec<DownloadTask> (all services, with file_extension)
  │ (scan_existing)
  ▼
ExistingFiles
  │ (build_sync_plan)
  ▼
SyncPlan
  │ (execute_downloads per service batch)
  ▼
SyncResult (aggregated across services)
```

## Data Volume Assumptions

- Typical Bandcamp collection: 50-500 albums
- collection_items pagination: 100 items/page
- Download page fetch: 1 HTTP request per album
- Album ZIP size: 50-200 MB typical
- Rate limit: max 3 req/s to Bandcamp
