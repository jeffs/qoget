# Bandcamp Web API Contract

**Date**: 2026-02-14 | **Source**: Reverse-engineered from existing tools

All endpoints require the `identity` cookie. No official API documentation exists.

## Authentication

**Cookie**: `identity` on domain `bandcamp.com`

All requests must include:
```
Cookie: identity={url_encoded_cookie_value}
User-Agent: Mozilla/5.0 (compatible; realistic browser UA)
```

## Endpoints

### 1. Collection Summary

Verify authentication and get fan_id.

```
GET https://bandcamp.com/api/fan/2/collection_summary
Cookie: identity=...
```

**Response** `200 OK`:
```json
{
  "fan_id": 1712700664,
  "collection_summary": {
    "fan_id": 1712700664,
    "tralbum_lookup": {
      "a1234567": {
        "item_id": 1234567,
        "band_id": 9876543,
        "purchased": "30 Jan 2026 02:51:12 GMT",
        "item_type": "album"
      }
    },
    "url": "https://bandcamp.com/username",
    "username": "username"
  }
}
```

**Error** `401/403`: Cookie invalid or expired.

---

### 2. Collection Items (paginated)

List all purchased items.

```
POST https://bandcamp.com/api/fancollection/1/collection_items
Cookie: identity=...
Content-Type: application/json
```

**Request body**:
```json
{
  "fan_id": "1712700664",
  "older_than_token": "1707955200:0:a::",
  "count": 100
}
```

| Field | Type | Description |
|-------|------|-------------|
| `fan_id` | string | Fan's numeric ID |
| `older_than_token` | string | Pagination cursor. First page: `"{now}:0:a::"` |
| `count` | integer | Items per page (max ~5000, recommend 100) |

**Response** `200 OK`:
```json
{
  "more_available": true,
  "last_token": "1707955200:1234567890:a::",
  "redownload_urls": {
    "a1234567": "https://bandcamp.com/download/album?id=1234567&sig=abc&sitem_id=9876&ts=1707955200.0",
    "t7654321": "https://bandcamp.com/download/track?id=7654321&sig=def&sitem_id=5432&ts=1707955200.0"
  },
  "items": [
    {
      "band_name": "Artist Name",
      "item_title": "Album Title",
      "item_id": 1234567,
      "item_type": "album",
      "sale_item_type": "a",
      "sale_item_id": 1234567,
      "token": "1707955200:1234567890:a::",
      "tralbum_type": "a",
      "purchased": "30 Jan 2026 02:51:12 GMT"
    }
  ]
}
```

| Response Field | Type | Description |
|----------------|------|-------------|
| `more_available` | bool | More pages exist |
| `last_token` | string | Next page cursor |
| `redownload_urls` | object | `"{type}{id}" → download_page_url` |
| `items[].sale_item_type` | string | `"a"` = album, `"t"` = track |
| `items[].token` | string | Per-item pagination token |

**Pagination**: Use `items.last().token` as `older_than_token`. Stop when `items` is empty.

---

### 3. Hidden Items (paginated)

Same as collection_items but for hidden purchases.

```
POST https://bandcamp.com/api/fancollection/1/hidden_items
```

Same request/response format as collection_items.

---

### 4. Download Page

Get format-specific download URLs for a purchase.

```
GET {redownload_url}
Cookie: identity=...
```

Where `redownload_url` is from `collection_items.redownload_urls`.

**Response**: HTML containing `<div id="pagedata" data-blob="{html_entity_encoded_json}">`.

Parsed JSON structure (relevant fields):
```json
{
  "digital_items": [
    {
      "downloads": {
        "aac-hi":        { "url": "https://popplers5.bandcamp.com/download/album?enc=aac-hi&fsig=...&id=...&ts=...", "size_mb": "90.5MB" },
        "mp3-320":       { "url": "https://popplers5.bandcamp.com/download/album?enc=mp3-320&...", "size_mb": "120.1MB" },
        "flac":          { "url": "https://popplers5.bandcamp.com/download/album?enc=flac&...", "size_mb": "350.2MB" }
      },
      "item_id": 1234567,
      "title": "Album Title",
      "artist": "Artist Name",
      "download_type": "a",
      "download_type_str": "album",
      "item_type": "album"
    }
  ]
}
```

| Format Key | Extension | Description |
|------------|-----------|-------------|
| `aac-hi` | `.m4a` | AAC high quality (target) |
| `mp3-320` | `.mp3` | 320 kbps MP3 |
| `flac` | `.flac` | Lossless |
| `mp3-v0` | `.mp3` | Variable bitrate MP3 |
| `vorbis` | `.ogg` | Ogg Vorbis |
| `alac` | `.m4a` | Apple Lossless |
| `wav` | `.wav` | Uncompressed |
| `aiff-lossless` | `.aiff` | Uncompressed |

---

### 5. File Download

Download the actual audio file(s).

```
GET {download_url}
Cookie: identity=...
```

Where `download_url` is from `digital_items[0].downloads["aac-hi"].url`.

**Response**:
- Albums: ZIP file (`Content-Type: application/zip`) containing individual audio files
- Tracks: bare audio file (`Content-Type: audio/mp4` for AAC)
- `Content-Disposition` header includes suggested filename

**ZIP structure** (albums):
```
Artist Name - Album Title/
  01 Track One.m4a
  02 Track Two.m4a
  ...
```

---

## Rate Limiting

- **Observed limit**: ~3 requests/second
- **429 response**: Back off 10 seconds, retry
- **Recommendation**: Use token-bucket rate limiter (3 req/s burst, 1 req/s sustained)

## Error Handling

| Status | Meaning | Action |
|--------|---------|--------|
| 200 | Success | Process response |
| 401/403 | Cookie invalid/expired | Report auth error, skip service |
| 429 | Rate limited | Back off 10s, retry (max 3) |
| 500/502/503 | Server error | Exponential backoff, retry (max 3) |
