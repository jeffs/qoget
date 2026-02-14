# Quickstart: Bandcamp Support & Unified CLI

**Branch**: `002-bandcamp-unified-cli` | **Date**: 2026-02-14

## Prerequisites

- Rust toolchain (existing)
- A Bandcamp account with purchases
- The `identity` cookie value from your browser

## Getting Your Bandcamp Cookie

1. Log in to [bandcamp.com](https://bandcamp.com) in your browser
2. Open Developer Tools (F12 or Cmd+Option+I)
3. Go to Application > Storage > Cookies > bandcamp.com
4. Find the `identity` cookie and copy its value

## Configuration

Add to `~/.config/qoget/config.toml`:

```toml
# Existing Qobuz config (unchanged)
[qobuz]
username = "your@email.com"
password = "your-password"

# New Bandcamp config
[bandcamp]
identity_cookie = "paste-your-identity-cookie-here"
```

Or via environment variable:
```sh
export BANDCAMP_IDENTITY="paste-your-identity-cookie-here"
```

### Backward Compatibility

Existing flat config still works for Qobuz-only users:
```toml
username = "your@email.com"
password = "your-password"
```

## Usage

### Sync all configured services
```sh
qoget sync ~/Music
```

### Sync only Bandcamp
```sh
qoget sync --service bandcamp ~/Music
```

### Sync only Qobuz (existing behavior)
```sh
qoget sync --service qobuz ~/Music
```

### Dry run (preview)
```sh
qoget sync --dry-run ~/Music
```

## Output Structure

```
~/Music/
├── Artist A/
│   ├── Album X/
│   │   ├── 01 - Song One.mp3       # from Qobuz
│   │   ├── 01 - Song One.m4a       # from Bandcamp (if same album purchased on both)
│   │   └── 02 - Song Two.mp3
│   └── Album Y/
│       ├── 01 - Track.m4a          # Bandcamp-only purchase
│       └── 02 - Track.m4a
└── Artist B/
    └── Album Z/
        └── 01 - Track.mp3          # Qobuz-only purchase
```

## New Dependencies

- `zip` — ZIP archive extraction (for Bandcamp album downloads)
- `scraper` or regex-based HTML parsing — extract `data-blob` JSON from download pages

## Running Tests

```sh
cargo test
```

## Development Notes

- Bandcamp rate limit: max ~3 req/s. The client includes a built-in rate limiter.
- Bandcamp albums download as ZIP files; the tool extracts individual tracks automatically.
- The `identity` cookie is long-lived but can expire. If you see auth errors, refresh the cookie from your browser.
