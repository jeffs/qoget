# Qobuz API Research

Findings compiled from source code analysis of: [streamrip](https://github.com/nathom/streamrip), [qobuz-dl](https://github.com/vitiko98/qobuz-dl), [QobuzApiSharp](https://github.com/DJDoubleD/QobuzApiSharp), [qobuz-api-rust](https://github.com/loxoron218/qobuz-api-rust), [gobuz](https://github.com/markhc/gobuz), [python-qobuz](https://github.com/taschenb/python-qobuz), and the [Kodi Qobuz plugin](https://github.com/tidalf/plugin.audio.qobuz).

## Base URL

```
https://www.qobuz.com/api.json/0.2
```

All endpoints below are relative to this base.

---

## 1. Authentication Flow

### Headers (all requests)

```
X-App-Id: <app_id>
```

After login, add:

```
X-User-Auth-Token: <user_auth_token>
```

### Login Endpoint

```
GET /user/login
```

**Option A -- Email + password:**

| Parameter  | Value                            |
|------------|----------------------------------|
| `email`    | user's email address             |
| `password` | MD5 hex digest of plaintext password |
| `app_id`   | 9-digit application ID           |

Password hashing (the API expects the MD5 hex string, not the raw password):

```rust
let password_hash = format!("{:x}", md5::compute(plaintext_password.as_bytes()));
```

**Option B -- Username + password:**

Same as above but use `username` instead of `email`.

**Option C -- Token re-login (session restore):**

| Parameter         | Value                       |
|-------------------|-----------------------------|
| `user_id`         | numeric user ID from prior login |
| `user_auth_token` | token from prior login      |
| `app_id`          | 9-digit application ID      |

### Login Response (abbreviated)

```json
{
  "user_auth_token": "AAABBBCCC...",
  "user": {
    "id": 12345678,
    "login": "username",
    "email": "user@example.com",
    "display_name": "Display Name",
    "firstname": "First",
    "lastname": "Last",
    "country_code": "US",
    "credential": {
      "id": 4,
      "label": "Sublime+",
      "description": "...",
      "parameters": {
        "lossy_streaming": true,
        "lossless_streaming": true,
        "hires_streaming": true,
        "hires_purchases_streaming": true,
        "mobile_streaming": true,
        "offline_streaming": true,
        "hfp_purchase": true,
        "included_format_group_ids": [1, 2, 4, 5],
        "color_scheme": { "logo": "#..." },
        "label": "Sublime+",
        "short_label": "SUB+",
        "source": "collector"
      }
    },
    "subscription": {
      "offer": "...",
      "start_date": "2024-01-01",
      "end_date": "2025-01-01",
      "is_canceled": false
    }
  }
}
```

**Critical field:** `user.credential.parameters` -- if this is `null` or empty, the account is a free account and cannot stream/download. Streamrip checks this and raises `IneligibleError` for free accounts. However, free accounts with purchases *may* still be able to download purchased content using `intent=download`.

---

## 2. Listing Purchases

### Endpoint

```
GET /purchase/getUserPurchases
```

Requires `X-User-Auth-Token` header.

### Parameters

| Parameter       | Required | Description                                  |
|-----------------|----------|----------------------------------------------|
| `limit`         | No       | Max items per page (default: 50)             |
| `offset`        | No       | Pagination offset (default: 0)               |
| `order_id`      | No       | Filter by specific order                     |
| `order_line_id` | No       | Filter by specific order line                |
| `flat`          | No       | Unknown; likely flattens nested structures   |

### Response Structure (inferred from multiple implementations)

```json
{
  "albums": {
    "offset": 0,
    "limit": 50,
    "total": 12,
    "items": [
      {
        "id": "album_id_string",
        "qobuz_id": 12345,
        "title": "Album Title",
        "version": null,
        "artist": {
          "id": 67890,
          "name": "Artist Name",
          "slug": "artist-name"
        },
        "artists": [
          { "id": 67890, "name": "Artist Name", "roles": ["main-artist"] }
        ],
        "label": {
          "id": 1234,
          "name": "Label Name",
          "slug": "label-name"
        },
        "genre": {
          "id": 119,
          "name": "Rock",
          "slug": "rock",
          "path": [112, 119],
          "color": "#5eabc1"
        },
        "image": {
          "small": "https://static.qobuz.com/images/covers/.../230.jpg",
          "thumbnail": "https://static.qobuz.com/images/covers/.../50.jpg",
          "large": "https://static.qobuz.com/images/covers/.../600.jpg",
          "back": null
        },
        "release_date_original": "2023-10-27",
        "release_date_download": "2023-10-27",
        "upc": "0123456789012",
        "duration": 2383,
        "tracks_count": 12,
        "media_count": 1,
        "maximum_bit_depth": 24,
        "maximum_sampling_rate": 96.0,
        "maximum_channel_count": 2,
        "hires": true,
        "hires_streamable": true,
        "streamable": true,
        "downloadable": true,
        "purchasable": true,
        "hires_purchased": true,
        "copyright": "2023 Label Name",
        "parental_warning": false
      }
    ]
  },
  "tracks": {
    "offset": 0,
    "limit": 50,
    "total": 3,
    "items": [
      { "...same shape as track objects below..." }
    ]
  }
}
```

### Pagination

All collection endpoints use `limit` + `offset`. Default limit is 50. Maximum observed limit is 500 (streamrip uses 500). Increment `offset` by `limit` until `offset >= total`.

```rust
let mut offset = 0;
let limit = 500;
loop {
    let page = api.get("/purchase/getUserPurchases", &[
        ("limit", &limit.to_string()),
        ("offset", &offset.to_string()),
    ]).await?;

    let albums = &page["albums"];
    let total = albums["total"].as_u64().unwrap();
    // process albums["items"]

    offset += limit;
    if offset >= total { break; }
}
```

---

## 3. Getting Album Metadata (with tracks)

### Endpoint

```
GET /album/get
```

### Parameters

| Parameter  | Required | Description                |
|------------|----------|----------------------------|
| `album_id` | Yes      | Album ID (string or int)   |
| `limit`    | No       | Track pagination limit     |
| `offset`   | No       | Track pagination offset    |

### Response (key fields)

The album object (same shape as above) plus an embedded `tracks` object:

```json
{
  "...album fields...",
  "tracks": {
    "offset": 0,
    "limit": 50,
    "total": 12,
    "items": [
      {
        "id": 216020864,
        "title": "Track Title",
        "version": null,
        "track_number": 9,
        "media_number": 1,
        "duration": 147,
        "isrc": "USMRG2384109",
        "performer": {
          "id": 384672,
          "name": "Artist Name"
        },
        "performers": "Producer Name, Producer - Artist Name, MainArtist - Composer Name, Composer, Lyricist",
        "composer": {
          "id": 334487,
          "name": "Composer Name"
        },
        "work": null,
        "audio_info": {
          "replaygain_track_gain": -7.08,
          "replaygain_track_peak": 0.936676
        },
        "copyright": "2023 Label Name",
        "maximum_bit_depth": 24,
        "maximum_sampling_rate": 96.0,
        "maximum_channel_count": 2,
        "hires": true,
        "hires_streamable": true,
        "streamable": true,
        "downloadable": true,
        "purchasable": true,
        "previewable": true,
        "sampleable": true,
        "displayable": true,
        "purchasable_at": 1698390000,
        "streamable_at": 1698390000,
        "parental_warning": false,
        "release_date_original": null,
        "release_date_download": null,
        "release_date_stream": null
      }
    ]
  }
}
```

**Key fields for sync:**
- `track_number` -- track position within a disc
- `media_number` -- disc number (1-based)
- `duration` -- seconds
- `isrc` -- International Standard Recording Code (unique per recording)
- `id` -- numeric track ID needed for download

---

## 4. Getting Download URLs

### Endpoint

```
GET /track/getFileUrl
```

This is the **only** endpoint that requires request signing.

### Parameters

| Parameter     | Required | Description                       |
|---------------|----------|-----------------------------------|
| `track_id`    | Yes      | Numeric track ID                  |
| `format_id`   | Yes      | Audio quality (see table below)   |
| `intent`      | Yes      | `"stream"`, `"download"`, or `"import"` |
| `request_ts`  | Yes      | Unix timestamp (seconds, as float or int) |
| `request_sig` | Yes      | MD5 signature (see below)         |

### Format IDs

| `format_id` | Quality                        | Extension |
|-------------|--------------------------------|-----------|
| `5`         | MP3 320 kbps CBR               | `.mp3`    |
| `6`         | FLAC 16-bit / 44.1 kHz (CD)   | `.flac`   |
| `7`         | FLAC 24-bit up to 96 kHz      | `.flac`   |
| `27`        | FLAC 24-bit up to 192 kHz     | `.flac`   |

### Intent Parameter

- `"stream"` -- for streaming playback; falls back to lower quality if requested quality is unavailable.
- `"download"` -- for downloading purchased content; **does NOT fall back** -- the exact `format_id` must match what was purchased. Format 27 reportedly does not work with `intent=download`. Format 7 only works for hi-res purchases.
- `"import"` -- purpose unclear; documented in python-qobuz.

For syncing purchases, use `intent=download` with the appropriate `format_id` matching the purchase quality. For MP3 320 downloads, `format_id=5` is the safe choice.

### Request Signature Generation

The signature is an MD5 hash of a specific concatenation:

```
trackgetFileUrlformat_id{format_id}intentstreamtrack_id{track_id}{timestamp}{app_secret}
```

**Concrete example in Rust:**

```rust
use md5;

fn generate_request_sig(
    track_id: &str,
    format_id: &str,  // "5", "6", "7", or "27"
    timestamp: &str,   // unix timestamp as string
    app_secret: &str,
) -> String {
    let data = format!(
        "trackgetFileUrlformat_id{format_id}intentstreamtrack_id{track_id}{timestamp}{app_secret}"
    );
    format!("{:x}", md5::compute(data.as_bytes()))
}
```

**IMPORTANT:** The signature string always uses `intentstream` regardless of the actual `intent` parameter value sent in the request. Every implementation hardcodes `"stream"` in the signature even when the `intent` query parameter is `"download"`. (Confirmed across streamrip, QobuzApiSharp, gobuz, and the Kodi plugin.)

### Response

```json
{
  "track_id": 216020864,
  "duration": 147,
  "url": "https://streaming-qobuz-std.akamaized.net/file?uid=...",
  "format_id": 5,
  "mime_type": "audio/mpeg",
  "sampling_rate": 44.1,
  "bit_depth": 16,
  "status": "ok"
}
```

On error (e.g., non-streamable or restricted):

```json
{
  "status": "error",
  "code": "NotAvailableForStreaming",
  "message": "This track is not available for streaming",
  "restrictions": [
    { "code": "FormatRestrictedByFormatAvailability" }
  ]
}
```

The `url` field is the direct download URL. It is a temporary signed URL that expires (exact TTL unknown, but treated as single-use in all implementations).

---

## 5. App ID and App Secret Discovery

The `app_id` (9-digit number) and `app_secret` (32-char hex string) are embedded in the Qobuz web player's JavaScript bundle. Third-party tools extract them at runtime via the following algorithm:

### Step 1: Fetch the login page

```
GET https://play.qobuz.com/login
```

### Step 2: Extract the bundle.js URL

Parse the HTML for:

```regex
<script src="(/resources/\d+\.\d+\.\d+-[a-z]\d{3}/bundle\.js)"></script>
```

This yields a path like `/resources/6.1.0-b123/bundle.js`.

### Step 3: Fetch the bundle

```
GET https://play.qobuz.com/resources/6.1.0-b123/bundle.js
```

### Step 4: Extract app_id

Search the bundle for:

```regex
production:\{api:\{appId:"(?P<app_id>\d{9})",appSecret:"(\w{32})"
```

The `app_id` group is the 9-digit ID. The `appSecret` captured here is **not** the one used for request signing -- it is a different value.

### Step 5: Extract the real signing secrets

The actual secrets used for `request_sig` are constructed from seed/info/extras values scattered throughout the bundle:

**5a.** Find seed + timezone pairs:

```regex
[a-z]\.initialSeed\("(?P<seed>[\w=]+)",window\.utimezone\.(?P<timezone>[a-z]+)\)
```

**5b.** Swap the order of the first two pairs (Qobuz uses JavaScript ternary conditions that always evaluate to false, making the second option execute first).

**5c.** Find info + extras for each timezone:

```regex
name:"\w+/(?P<timezone>Berlin|London|...)",info:"(?P<info>[\w=]+)",extras:"(?P<extras>[\w=]+)"
```

(Timezone names are capitalized versions of what was found in step 5a.)

**5d.** Concatenate `seed + info + extras`, strip the last 44 characters, and base64-decode:

```python
secret = base64.b64decode(
    (seed + info + extras)[:-44]
).decode("utf-8")
```

This yields typically 2 secret strings. Test each against a known track to find the working one.

### Secret Validation

Test each secret by requesting a file URL for a known track (streamrip uses track ID `19512574` at quality 4):

```
GET /track/getFileUrl?track_id=19512574&format_id=27&intent=stream&request_ts=...&request_sig=...
```

- Status 200 or 401 -> secret is valid (401 means auth issue, not secret issue)
- Status 400 -> secret is invalid, try the next one

### Stability Warning

This extraction algorithm breaks whenever Qobuz updates their web player bundle structure. This is the most fragile part of any third-party integration. The regex patterns and the seed/info/extras concatenation logic have changed multiple times historically.

---

## 6. Other Useful Endpoints

### User Favorites

```
GET /favorite/getUserFavorites
```

| Parameter | Value                              |
|-----------|------------------------------------|
| `type`    | `"albums"`, `"tracks"`, or `"artists"` |
| `limit`   | pagination limit                   |
| `offset`  | pagination offset                  |

Response: `{ "albums": { "items": [...], "total": N, "limit": L, "offset": O } }` (or `tracks`/`artists` depending on `type`).

### Search

```
GET /{media_type}/search
```

Where `{media_type}` is `album`, `track`, `artist`, or `playlist`.

| Parameter | Value          |
|-----------|----------------|
| `query`   | search string  |
| `limit`   | pagination     |
| `offset`  | pagination     |

### Artist Albums

```
GET /artist/get
```

| Parameter   | Value                  |
|-------------|------------------------|
| `artist_id` | numeric artist ID      |
| `extra`     | `"albums"`             |
| `limit`     | pagination             |
| `offset`    | pagination             |

---

## 7. Rate Limiting and Gotchas

### Rate Limits

- **No documented rate limit.** Qobuz does not publish rate limit headers or documentation.
- Streamrip defaults to **60 requests per minute** as a self-imposed limit with a configurable `requests_per_minute` setting.
- Streamrip limits concurrent downloads to **6 connections**.
- No evidence of HTTP 429 responses in any implementation's error handling, but prudent to implement throttling anyway.

### Session Expiry

- The `user_auth_token` is long-lived but its exact TTL is unknown.
- Official guidance: "You should not store any user password in your application but the user_auth_token instead, which will remain valid until the user revokes your application."
- Best practice: store the token and re-use it, falling back to re-login on 401.

### Download URL Expiry

- The URL returned by `track/getFileUrl` is temporary and signed.
- Treat it as single-use: request a fresh URL for each download.
- Do not cache or reuse these URLs.

### Free Account Restrictions

- Free accounts have `user.credential.parameters` set to `null` in the login response.
- Streamrip rejects free accounts outright. However, free accounts that have purchased music *can* download those purchases using `intent=download` with the correct `format_id`.

### Intent vs. Format Compatibility

- `intent=stream` with any `format_id`: API will return the best available quality up to the requested format. Falls back gracefully.
- `intent=download` with `format_id`: **must exactly match** the purchased quality. No fallback. `format_id=27` reportedly never works with `intent=download`. `format_id=7` only works for hi-res purchases.
- **For MP3 320 sync of purchases:** use `intent=download` with `format_id=5`.

### Password Format

- The API expects an MD5 hex digest of the plaintext password, **not** the plaintext password itself.
- All implementations hash the password client-side before sending.

### Request Method

- All API calls use **HTTP GET** with query parameters (not POST bodies), despite some implementations (like the Kodi plugin) using POST with form-encoded data to the same endpoints. Both appear to work.

### Bundle.js Fragility

- The app_id/secret extraction from `play.qobuz.com`'s bundle.js breaks periodically as Qobuz deploys new frontend versions.
- Multiple projects (streamrip, qobuz-dl) have experienced breakage and needed regex updates.
- Consider caching app_id and secrets and only re-extracting on authentication failure.

### Signature Quirk

- The `request_sig` for `track/getFileUrl` always embeds `intentstream` in the hash input string, even when the actual `intent` query parameter is `"download"`. This is confirmed across all implementations.

---

## 8. Minimal Sync Workflow

1. **Bootstrap credentials:** Extract `app_id` and `secrets` from `play.qobuz.com/login` bundle.js (or use cached values).
2. **Login:** `GET /user/login` with email + MD5(password) + app_id. Store `user_auth_token`.
3. **Validate secret:** Test each secret against `track/getFileUrl` for a known track. Keep the working one.
4. **List purchases:** Paginate through `GET /purchase/getUserPurchases?limit=500&offset=0`. Collect album and track IDs.
5. **Get album metadata:** For each purchased album, `GET /album/get?album_id=X` to get the track listing with track numbers, disc numbers, durations, and track IDs.
6. **Download each track:** For each track, `GET /track/getFileUrl?track_id=X&format_id=5&intent=download&request_ts=T&request_sig=S`. Then HTTP GET the returned URL to download the file.
7. **Organize files:** Use album metadata (artist, album title, disc number, track number, track title) to build the filesystem path.

---

## Sources

- [streamrip (nathom)](https://github.com/nathom/streamrip) -- `streamrip/client/qobuz.py`: complete client implementation
- [qobuz-dl (vitiko98)](https://github.com/vitiko98/qobuz-dl) -- `qobuz_dl/bundle.py`: app_id/secret extraction
- [QobuzApiSharp (DJDoubleD)](https://github.com/DJDoubleD/QobuzApiSharp) -- C# implementation with signature generation
- [qobuz-api-rust (loxoron218)](https://github.com/loxoron218/qobuz-api-rust) -- Rust models and auth
- [gobuz (markhc)](https://pkg.go.dev/github.com/markhc/gobuz) -- Go implementation with format constants
- [python-qobuz (taschenb)](https://github.com/taschenb/python-qobuz) -- clean Python wrapper with format_id docs
- [Kodi Qobuz plugin (tidalf)](https://github.com/tidalf/plugin.audio.qobuz) -- `api/raw.py`: purchase endpoint, intent parameter
- [App ID extraction gist (vitiko98)](https://gist.github.com/vitiko98/bb89fd203d08e285d06abf40d96db592)
