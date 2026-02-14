# Data Model: Qobuz Purchase Sync CLI

**Feature Branch**: `001-qobuz-sync`
**Date**: 2026-02-14

## Domain Types

### AppCredentials

Extracted from the Qobuz web player bundle. Required for all API requests.

| Field       | Type     | Source            | Notes                                    |
|-------------|----------|-------------------|------------------------------------------|
| `app_id`    | `String` | bundle.js         | 9-digit numeric string                   |
| `app_secret`| `String` | bundle.js         | 32-char hex string, used for signing     |

### UserAuth

Obtained from the login endpoint. Represents an authenticated session.

| Field            | Type     | Source          | Notes                               |
|------------------|----------|-----------------|--------------------------------------|
| `user_auth_token`| `String` | login response  | Long-lived, revocable by user        |
| `user_id`        | `u64`    | login response  | Numeric user identifier              |

### Session

Combines app credentials and user auth into a valid API session. Cannot be constructed without both.

| Field         | Type             | Notes                                     |
|---------------|------------------|-------------------------------------------|
| `credentials` | `AppCredentials` | App ID + secret for signing               |
| `auth`        | `UserAuth`       | User token for authenticated requests     |

### Album

Represents a purchased album as returned by the Qobuz API.

| Field                | Type              | Source                    | Notes                                    |
|----------------------|-------------------|---------------------------|------------------------------------------|
| `id`                 | `AlbumId(String)` | `id` or `qobuz_id`       | Album identifier                         |
| `title`              | `String`          | `title`                   | Album title                              |
| `version`            | `Option<String>`  | `version`                 | e.g., "Deluxe Edition", often null       |
| `artist`             | `Artist`          | `artist`                  | Primary album artist                     |
| `media_count`        | `u8`              | `media_count`             | Number of discs (1 = single disc)        |
| `tracks_count`       | `u16`             | `tracks_count`            | Total track count across all discs       |
| `tracks`             | `Vec<Track>`      | `tracks.items` (via /album/get) | Populated after metadata fetch  |
| `release_date`       | `Option<String>`  | `release_date_original`   | ISO date string                          |

**Identity**: `AlbumId` (newtype over `String`). Unique per album.

### Track

Represents an individual audio track within an album or as a standalone purchase.

| Field            | Type                  | Source            | Notes                                    |
|------------------|-----------------------|-------------------|------------------------------------------|
| `id`             | `TrackId(u64)`        | `id`              | Numeric track ID, used for download URLs |
| `title`          | `String`              | `title`           | Track title                              |
| `track_number`   | `TrackNumber(u8)`     | `track_number`    | Position within disc (1-based)           |
| `disc_number`    | `DiscNumber(u8)`      | `media_number`    | Disc number (1-based)                    |
| `duration`       | `u32`                 | `duration`        | Duration in seconds                      |
| `performer`      | `Artist`              | `performer`       | Track-level artist (may differ from album artist) |
| `isrc`           | `Option<String>`      | `isrc`            | International Standard Recording Code    |

**Identity**: `TrackId` (newtype over `u64`). Unique per track globally.

### Artist

| Field  | Type     | Source  | Notes              |
|--------|----------|---------|--------------------|
| `id`   | `u64`    | `id`    | Numeric artist ID  |
| `name` | `String` | `name`  | Display name       |

### PurchaseList

Response from the purchases endpoint. Albums and tracks are paginated independently.

| Field            | Type         | Source            | Notes                       |
|------------------|--------------|-------------------|-----------------------------|
| `albums`         | `Vec<Album>` | `albums.items`    | All pages aggregated        |
| `tracks`         | `Vec<Track>` | `tracks.items`    | Standalone track purchases  |

### DownloadTask

A resolved download action: a track paired with its target filesystem path.

| Field         | Type       | Notes                                       |
|---------------|------------|---------------------------------------------|
| `track`       | `Track`    | The track to download                       |
| `album`       | `Album`    | Parent album (for artist/title context)      |
| `target_path` | `PathBuf`  | Fully resolved local path for the MP3 file  |

### SkippedTrack

A track that was not downloaded, with the reason.

| Field         | Type         | Notes                                   |
|---------------|--------------|-----------------------------------------|
| `track`       | `Track`      | The track that was skipped              |
| `target_path` | `PathBuf`    | Where it would have been written        |
| `reason`      | `SkipReason` | Why it was skipped                      |

### SkipReason (enum)

| Variant          | Meaning                                            |
|------------------|----------------------------------------------------|
| `AlreadyExists`  | File exists with correct size                      |
| `DryRun`         | Dry-run mode, download suppressed                  |

### SyncPlan

The output of comparing remote purchases against local state. Separates actions at the type level.

| Field         | Type                | Notes                                  |
|---------------|---------------------|----------------------------------------|
| `downloads`   | `Vec<DownloadTask>` | Tracks that need to be downloaded      |
| `skipped`     | `Vec<SkippedTrack>` | Tracks already present locally         |
| `total_tracks`| `usize`             | Total tracks in the purchase library   |

### SyncResult

The outcome of executing a sync plan.

| Field       | Type                  | Notes                                   |
|-------------|-----------------------|-----------------------------------------|
| `succeeded` | `Vec<DownloadTask>`   | Successfully downloaded tracks          |
| `failed`    | `Vec<DownloadError>`  | Tracks that failed with error context   |
| `skipped`   | `Vec<SkippedTrack>`   | Tracks that were skipped                |

### DownloadError

| Field    | Type           | Notes                              |
|----------|----------------|------------------------------------|
| `task`   | `DownloadTask` | The download that failed           |
| `error`  | `String`       | Human-readable error description   |

### Config

User-provided configuration, merged from config file and environment variables.

| Field      | Type              | Source                                         | Notes                                |
|------------|-------------------|------------------------------------------------|--------------------------------------|
| `username` | `String`          | `QOBUZ_USERNAME` env or config `username`      | Email or username                    |
| `password` | `String`          | `QOBUZ_PASSWORD` env or config `password`      | Plaintext (hashed before API call)   |
| `app_id`   | `Option<String>`  | config `app_id`                                | Override auto-extracted app_id       |
| `app_secret`| `Option<String>` | config `app_secret`                            | Override auto-extracted secret       |

## Newtype Wrappers

These enforce domain invariants at compile time (parse, don't validate):

| Newtype        | Inner | Invariant                     |
|----------------|-------|-------------------------------|
| `TrackId`      | `u64` | Nonzero, from API             |
| `AlbumId`      | `String` | Non-empty, from API        |
| `TrackNumber`  | `u8`  | 1-based (from API `track_number`)  |
| `DiscNumber`   | `u8`  | 1-based (from API `media_number`)  |

## State Transitions

### Sync Lifecycle

```
Config → Session → PurchaseList → SyncPlan → SyncResult
  │         │           │              │           │
  │     (login)    (fetch+paginate)  (diff)    (download)
  │                                              │
  └── Config loaded from file/env                └── Exit code: 0 if all succeeded, 1 if any failed
```

This is a linear pipeline with no branching state. Each stage produces a value consumed by the next. No mutable global state.

## Filesystem Layout (output)

```
<target_dir>/
├── Artist Name/
│   ├── Album Title/
│   │   ├── 01 - Track Title.mp3
│   │   ├── 02 - Track Title.mp3
│   │   └── ...
│   └── Multi-Disc Album/
│       ├── Disc 1/
│       │   ├── 01 - Track Title.mp3
│       │   └── ...
│       └── Disc 2/
│           ├── 01 - Track Title.mp3
│           └── ...
└── Various Artists/
    └── Compilation Album/
        ├── 01 - Track Artist - Track Title.mp3
        └── ...
```

### Path Sanitization Rules

Characters replaced or removed from artist names, album titles, and track titles:
- `/` and `\` → `-` (path separators)
- `:` → `-` (invalid on Windows/macOS)
- `*`, `?`, `"`, `<`, `>`, `|` → removed (invalid on Windows)
- Leading/trailing whitespace → trimmed
- Leading `.` → removed (hidden files on Unix)
- Consecutive spaces → collapsed to single space
- Total path component length capped at 255 bytes (filesystem limit)
