# Research: Bandcamp Support & Unified CLI

**Branch**: `002-bandcamp-unified-cli` | **Date**: 2026-02-14

## 1. Bandcamp Internal API

### Decision: Use cookie-based web API (no official API exists for purchased content)
### Rationale: All existing Bandcamp download tools (bandcampsync, bandsnatch, bandcamp-collection-downloader) use the same cookie-based approach. Bandcamp has no public API for accessing purchased downloads.
### Alternatives: Bandcamp has a label/artist API at `bandcamp.com/developer` but it does not support fan purchase/download operations.

### Authentication

- **Mechanism**: `identity` cookie from `bandcamp.com`
- **Format**: URL-encoded string containing: version number, hex token, JSON payload with `id` (fan_id), `h1` (hash), `ex` (expiry flag), tab-separated
- **Usage**: Set as standard `Cookie: identity=...` header on all requests
- **Longevity**: Long-lived (months), invalidated only by logout or password change
- **fan_id extraction**: Decoded from the cookie JSON payload, or via `GET /api/fan/2/collection_summary`

### Purchase Listing

**Endpoint**: `POST https://bandcamp.com/api/fancollection/1/collection_items`

Request:
```json
{
  "fan_id": "1234567890",
  "older_than_token": "{unix_timestamp}:0:a::",
  "count": 100
}
```

Response:
```json
{
  "more_available": true,
  "last_token": "1707955200:1234567890:a::",
  "redownload_urls": {
    "a1234567": "https://bandcamp.com/download/album?id=1234567&sig=...&sitem_id=...&ts=...",
    "t7654321": "https://bandcamp.com/download/track?id=7654321&sig=..."
  },
  "items": [
    {
      "band_name": "Artist Name",
      "item_title": "Album Title",
      "item_id": 1234567,
      "item_type": "album",
      "sale_item_type": "a",
      "sale_item_id": 1234567,
      "token": "1707955200:1234567890:a::"
    }
  ]
}
```

Pagination: use `token` from last item as `older_than_token` for next page. Stop when `items` is empty.

Hidden items: `POST /api/fancollection/1/hidden_items` — same format.

### Download Flow (3 phases)

**Phase 1**: Get download page from `redownload_url`
```
GET https://bandcamp.com/download/album?id=...&sig=...&sitem_id=...&ts=...
Cookie: identity=...
```
Response: HTML with `<div id="pagedata" data-blob="...">` containing JSON with `digital_items`.

**Phase 2**: Extract format-specific download URL from `digital_items[0].downloads["aac-hi"].url`

**Phase 3**: GET the download URL directly (with cookies). Bandcamp handles the redirect server-side.
- Albums: returned as `.zip` files containing individual tracks
- Single tracks: returned as bare audio files

Alternative Phase 3: Replace `/download/` with `/statdownload/`, append `&.vrs=1&.rand={random}`, parse JS-wrapped JSON to get final CDN URL. More robust but more complex.

### Available Formats

| Key | Extension | Description |
|-----|-----------|-------------|
| `aac-hi` | `.m4a` | AAC high quality (target format) |
| `mp3-320` | `.mp3` | 320 kbps CBR MP3 |
| `flac` | `.flac` | Lossless FLAC |
| `mp3-v0` | `.mp3` | Variable bitrate MP3 |
| `vorbis` | `.ogg` | Ogg Vorbis |
| `alac` | `.m4a` | Apple Lossless |
| `wav` | `.wav` | Uncompressed |
| `aiff-lossless` | `.aiff` | Uncompressed |

### Rate Limiting

Bandcamp returns HTTP 429 under load. Existing tools cap at 3 requests/second with 10-second backoff on 429.

### Browser Impersonation

Some tools (bandcampsync, easlice/bandcamp-downloader) use TLS fingerprint impersonation (`impersonate="chrome"`). In Rust with `reqwest`, we may need to set a realistic User-Agent at minimum. The `reqwest` crate does not support TLS fingerprint impersonation natively; if Bandcamp enforces this, we may need `reqwest-impersonate` or similar.

---

## 2. ZIP Extraction for Album Downloads

### Decision: Extract individual tracks from album ZIP downloads
### Rationale: Bandcamp delivers albums as ZIP files, not individual track files. The ZIP contains one audio file per track, named with track numbers and titles. We need to extract, rename, and place individual tracks in the correct directory structure.
### Alternatives: Download tracks individually by scraping each track's page — much slower (N requests per album vs 1), and Bandcamp doesn't expose individual track downloads for album purchases in the same way.

### ZIP structure (observed from tools)
```
Artist Name - Album Title/
  01 Track One.m4a
  02 Track Two.m4a
  ...
```

Track files within the ZIP already include track numbers. We need to:
1. Download the ZIP to a temp file
2. Extract track files
3. Rename to match our naming convention (`NN - Title.m4a`)
4. Place in the correct artist/album directory
5. Clean up the ZIP

### Rust dependency
`zip` crate (well-maintained, pure Rust) for ZIP extraction.

---

## 3. Config Structure for Multi-Service

### Decision: Use TOML sections with backward-compatible flat key fallback
### Rationale: The existing flat config (`username`, `password`) must keep working (FR-012). TOML sections (`[qobuz]`, `[bandcamp]`) provide clean namespacing.
### Alternatives: Separate config files per service (more complex, poor UX), environment-only config (loses persistence).

### Proposed config format
```toml
# Backward-compatible: bare keys still work for Qobuz
username = "user@example.com"
password = "secret"

# Preferred: explicit sections
[qobuz]
username = "user@example.com"
password = "secret"
app_id = "123456789"       # optional
app_secret = "abc..."      # optional

[bandcamp]
identity_cookie = "6%09..."
```

### Environment variables
- `QOBUZ_USERNAME`, `QOBUZ_PASSWORD` — existing, unchanged
- `BANDCAMP_IDENTITY` — new, for the identity cookie value

### Precedence
1. Environment variables
2. Config file section (`[qobuz]`/`[bandcamp]`)
3. Config file bare keys (Qobuz only, for backward compat)
4. Interactive prompts (Qobuz only — Bandcamp cookie can't be prompted)

---

## 4. Service Trait Abstraction

### Decision: Introduce a `MusicService` trait, keep it minimal
### Rationale: The existing code has clean separation between client (Qobuz-specific), sync (pure), download (minimal client dependency), and path (pure) modules. A trait needs only to cover the client interface.
### Alternatives: Enum-based dispatch (simpler but closed), dynamic dispatch via `Box<dyn>` (flexible but loses static types). We'll use enum dispatch since we have a known, small set of services.

### Proposed trait boundary

The key realization: Bandcamp's download model is fundamentally different from Qobuz's. Qobuz provides individual track download URLs; Bandcamp provides album-level ZIP downloads. This means the download module can't be fully generic over a single `get_file_url()` method.

Instead of forcing a common trait, we normalize the output: each service produces a `Vec<DownloadTask>` with resolved `target_path` values. The download execution is service-specific (Qobuz streams individual files; Bandcamp downloads+extracts ZIPs). The sync plan (dedup, skip existing) remains generic.

### Architecture after refactoring

```
Config → [QobuzService, BandcampService] → unified PurchaseList per service
       → merged SyncPlan (file-path dedup across services)
       → service-specific download execution
       → unified SyncResult
```

---

## 5. Track Metadata from Bandcamp

### Decision: Extract artist/album/track metadata from collection_items API + download page
### Rationale: The `collection_items` response provides `band_name` and `item_title` (artist and album). Individual track titles come from the ZIP file contents or can be parsed from filenames within the ZIP.
### Alternatives: Scrape individual album pages for full track metadata — more data but much slower and unnecessary for file organization.

### Metadata mapping

| qoget field | Bandcamp source |
|-------------|-----------------|
| artist.name | `items[].band_name` |
| album.title | `items[].item_title` |
| track.title | Parsed from ZIP entry filename (strip track number prefix) |
| track.track_number | Parsed from ZIP entry filename (leading digits) |
| album.media_count | Always 1 (Bandcamp doesn't use multi-disc) |
| track.media_number | Always 1 |

Sources:
- [bandcampsync (meeb)](https://github.com/meeb/bandcampsync)
- [bandsnatch (Ovyerus)](https://github.com/Ovyerus/bandsnatch)
- [bandcamp-collection-downloader (Ezwen)](https://github.com/Ezwen/bandcamp-collection-downloader)
- [easlice/bandcamp-downloader](https://github.com/easlice/bandcamp-downloader)
- [Bandcamp-API OpenAPI spec (michaelherger)](https://github.com/michaelherger/Bandcamp-API)
- [Reverse engineering Bandcamp downloads (torunar)](https://torunar.github.io/en/2024/06/24/bandcamp-downloads/)
