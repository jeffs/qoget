# Qobuz API Contract

**Base URL**: `https://www.qobuz.com/api.json/0.2`
**Date**: 2026-02-14
**Source**: Reverse-engineered from streamrip, qobuz-dl, QobuzApiSharp, gobuz, and others. No official documentation exists.

## Headers

All requests:
```
X-App-Id: <app_id>
```

Authenticated requests (after login):
```
X-User-Auth-Token: <user_auth_token>
```

## Endpoints

### 1. Login

```
GET /user/login
```

**Parameters**:

| Name       | Type   | Required | Description                       |
|------------|--------|----------|-----------------------------------|
| `email`    | string | yes*     | User's email address              |
| `password` | string | yes      | MD5 hex digest of plaintext password |
| `app_id`   | string | yes      | 9-digit application ID            |

*Can use `username` instead of `email`.

**Response** (key fields):

```json
{
  "user_auth_token": "string",
  "user": {
    "id": 12345678,
    "login": "string",
    "email": "string",
    "credential": {
      "parameters": { "...or null for free accounts..." }
    }
  }
}
```

**Errors**: HTTP 401 for invalid credentials.

---

### 2. List Purchases

```
GET /purchase/getUserPurchases
```

**Auth**: Required (`X-User-Auth-Token`).

**Parameters**:

| Name     | Type | Required | Default | Description        |
|----------|------|----------|---------|--------------------|
| `limit`  | int  | no       | 50      | Items per page     |
| `offset` | int  | no       | 0       | Pagination offset  |

**Response**:

```json
{
  "albums": {
    "offset": 0,
    "limit": 500,
    "total": 42,
    "items": [
      {
        "id": "string",
        "qobuz_id": 12345,
        "title": "string",
        "version": "string | null",
        "artist": { "id": 67890, "name": "string" },
        "media_count": 1,
        "tracks_count": 12,
        "downloadable": true
      }
    ]
  },
  "tracks": {
    "offset": 0,
    "limit": 500,
    "total": 3,
    "items": [ "...track objects..." ]
  }
}
```

**Pagination**: Increment `offset` by `limit` until `offset >= total`. Max observed `limit`: 500.

---

### 3. Get Album (with tracks)

```
GET /album/get
```

**Auth**: Required.

**Parameters**:

| Name       | Type   | Required | Description          |
|------------|--------|----------|----------------------|
| `album_id` | string | yes      | Album identifier     |

**Response**: Full album object (same as purchase item) plus embedded `tracks`:

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
        "title": "string",
        "track_number": 9,
        "media_number": 1,
        "duration": 147,
        "performer": { "id": 384672, "name": "string" },
        "isrc": "string",
        "downloadable": true
      }
    ]
  }
}
```

**Key fields**: `track_number` (position in disc), `media_number` (disc number, 1-based).

---

### 4. Get Download URL (signed)

```
GET /track/getFileUrl
```

**Auth**: Required. **Signed**: Yes (only signed endpoint).

**Parameters**:

| Name          | Type   | Required | Description                          |
|---------------|--------|----------|--------------------------------------|
| `track_id`    | int    | yes      | Numeric track ID                     |
| `format_id`   | int    | yes      | Quality format (see below)           |
| `intent`      | string | yes      | `"download"` for purchased tracks    |
| `request_ts`  | string | yes      | Unix timestamp (seconds)             |
| `request_sig` | string | yes      | MD5 signature                        |

**Format IDs**:

| ID  | Quality              |
|-----|----------------------|
| 5   | MP3 320 kbps         |
| 6   | FLAC 16/44.1 (CD)    |
| 7   | FLAC 24/96           |
| 27  | FLAC 24/192          |

**Signature generation**:

```
MD5("trackgetFileUrlformat_id{format_id}intentstreamtrack_id{track_id}{timestamp}{app_secret}")
```

The signature **always** uses `intentstream` in the hash input. As of 2026-02, the server also validates the `intent` query parameter against the signature, so `intent=stream` must be used in the query as well. Using `intent=download` now returns HTTP 400 "Invalid Request Signature".

**Response**:

```json
{
  "track_id": 216020864,
  "url": "https://streaming-qobuz-std.akamaized.net/file?uid=...",
  "format_id": 5,
  "mime_type": "audio/mpeg",
  "sampling_rate": 44.1,
  "bit_depth": 16
}
```

**Errors**:

```json
{
  "status": "error",
  "code": "NotAvailableForStreaming",
  "message": "string"
}
```

The `url` is a temporary signed URL. Request a fresh URL for each download; do not cache.

---

## App Credential Extraction

The `app_id` and `app_secret` are embedded in the Qobuz web player's JavaScript bundle.

### Algorithm

1. `GET https://play.qobuz.com/login` -- extract bundle.js URL from HTML
2. `GET https://play.qobuz.com/resources/{version}/bundle.js` -- fetch bundle
3. Extract `app_id` via regex: `production:\{api:\{appId:"(\d{9})"`
4. Extract seed/timezone pairs via regex: `initialSeed\("([\w=]+)",window\.utimezone\.([\w]+)\)`
5. For each timezone, find info/extras: `name:"\w+/(?P<tz>...)",info:"([\w=]+)",extras:"([\w=]+)"`
6. Concatenate `seed + info + extras`, strip last 44 chars, base64-decode to get secret
7. Validate each candidate secret against `track/getFileUrl` for a known track

### Fragility

This extraction breaks when Qobuz updates their frontend bundle structure. The config file supports manual `app_id` and `app_secret` overrides as a fallback.

## Rate Limiting

- No documented rate limits from Qobuz
- Self-imposed: ~4 concurrent downloads, reasonable delays between API calls
- Implement exponential backoff on HTTP 429 or 5xx responses
