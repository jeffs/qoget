# qoget

A command-line tool that syncs your purchased music from [Qobuz](https://www.qobuz.com/) and [Bandcamp](https://bandcamp.com/) to a local directory, organized by artist and album.

## Quick start

```sh
cargo install --path .
```

Create `~/.config/qoget/config.toml`:

```toml
[qobuz]
username = "your-email@example.com"
password = "your-qobuz-password"

[bandcamp]
identity_cookie = "your-bandcamp-identity-cookie"
```

Then sync:

```sh
qoget sync ~/Music
```

Run it again after buying new music and it only downloads what's new.

## What it does

- Downloads your entire purchase library from configured services
  + Qobuz: MP3 320 (`.mp3`)
  + Bandcamp: AAC high quality (`.m4a`)
- Organizes files as `Artist/Album/01 - Track.ext`
  + Multi-disc albums as `Artist/Album/Disc 2/01 - Track.ext`
  + Compilations as `Various Artists/Album/01 - Miles Davis - So What.ext`
- Skips files that already exist locally (incremental sync)
- Downloads up to four tracks at a time with progress output (Qobuz)
- Retries on transient network errors
- Cleans up partial files if a download fails

## Options

```sh
qoget sync ~/Music                        # sync all configured services
qoget sync ~/Music --dry-run              # see what would be downloaded
qoget sync ~/Music --service qobuz        # sync only Qobuz
qoget sync ~/Music --service bandcamp     # sync only Bandcamp
```

## Configuration

Credentials can come from the config file, environment variables, or both. Environment variables take precedence.

### Qobuz

| Source | Fields |
|--------|--------|
| `~/.config/qoget/config.toml` | `[qobuz]` section: `username`, `password`, `app_id`\*, `app_secret`\* |
| Environment | `QOBUZ_USERNAME`, `QOBUZ_PASSWORD` |

\*`app_id` and `app_secret` are optional overrides. Normally these are extracted automatically from the Qobuz web player. If extraction breaks (Qobuz updated their frontend), you can set them manually.

Bare keys (without a `[qobuz]` section) are still supported for backward compatibility:

```toml
username = "your-email@example.com"
password = "your-qobuz-password"
```

### Bandcamp

| Source | Fields |
|--------|--------|
| `~/.config/qoget/config.toml` | `[bandcamp]` section: `identity_cookie` |
| Environment | `BANDCAMP_IDENTITY` |

To get your Bandcamp identity cookie:

1. Log in to [bandcamp.com](https://bandcamp.com) in your browser
2. Open developer tools (F12) and go to the Application/Storage tab
3. Find the `identity` cookie for `bandcamp.com`
4. Copy the cookie value (it's a URL-encoded string starting with a number)

## Building from source

Requires a recent Rust. Originally developed using 1.93.

```sh
git clone <this-repo> qoget
cd qoget
cargo build --release
mv target/release/qoget ~/.local/bin/  # or wherever you like
```

## How it works

### Qobuz

Qobuz doesn't have a public API. This tool uses the same endpoints as the Qobuz web player:

1. Extracts app credentials from `play.qobuz.com`'s JavaScript bundle
2. Logs in with your email + password
3. Fetches your purchase list and album metadata
4. Requests signed download URLs for each track
5. Downloads tracks in parallel to the target directory

The credential extraction step is the most fragile part. It parses JavaScript with regexes and will break when Qobuz updates their frontend. The `app_id`/`app_secret` config overrides exist for this reason.

### Bandcamp

Bandcamp also lacks a public API. This tool uses the same internal endpoints as the Bandcamp website:

1. Authenticates using your browser's `identity` cookie
2. Fetches your purchase list (collection items + hidden items)
3. For each album, fetches the download page and extracts the AAC download URL
4. Downloads album ZIP archives, extracts `.m4a` files, and places them in the target directory

Rate limiting is applied (3 requests/second) with automatic backoff on 429 responses.

## License

Personal project. Use at your own risk. Do what you want with it, but don't DoS Qobuz or Bandcamp, OK?
