# qoget

A command-line tool that syncs your purchased music from [Qobuz](https://www.qobuz.com/) to a local directory as MP3 320 files, organized by artist and album.

## Quick start

```sh
cargo install --path .
```

Create `~/.config/qoget/config.toml`:

```toml
username = "your-email@example.com"
password = "your-qobuz-password"
```

Then sync:

```sh
qoget sync ~/Music/Qobuz
```

Run it again after buying new music and it only downloads what's new.

## What it does

- Downloads your entire Qobuz purchase library as MP3 320
- Organizes files as `Artist/Album/01 - Track.mp3`
  + Multi-disc albums as `Artist/Album/Disc 2/01 - Track.mp3`
  + Compilations as `Various Artists/Album/01 - Miles Davis - So What.mp3`
- Skips files that already exist locally (incremental sync)
- Downloads up to four tracks at a time with progress output
- Retries on transient network errors
- Cleans up partial files if a download fails

## Options

```sh
qoget sync ~/Music/Qobuz              # full sync
qoget sync ~/Music/Qobuz --dry-run    # see what would be downloaded
```

## Configuration

Credentials can come from the config file, environment variables, or both. Environment variables take precedence.

| Source | Fields |
|--------|--------|
| `~/.config/qoget/config.toml` | `username`, `password`, `app_id`*, `app_secret`* |
| Environment | `QOBUZ_USERNAME`, `QOBUZ_PASSWORD` |

*`app_id` and `app_secret` are optional overrides. Normally these are extracted automatically from the Qobuz web player. If extraction breaks (Qobuz updated their frontend), you can set them manually.

## Building from source

Requires a recent Rust. Originally developed using 1.93.

```sh
git clone <this-repo> qoget
cd qoget
cargo build --release
mv target/release/qoget ~/.local/bin/  # or wherever you like
```

## How it works

Qobuz doesn't have a public API. This tool uses the same endpoints as the Qobuz web player:

1. Extracts app credentials from `play.qobuz.com`'s JavaScript bundle
2. Logs in with your email + password
3. Fetches your purchase list and album metadata
4. Requests signed download URLs for each track
5. Downloads tracks in parallel to the target directory

The credential extraction step is the most fragile part. It parses JavaScript with regexes and will break when Qobuz updates their frontend. The `app_id`/`app_secret` config overrides exist for this reason.

## License

Personal project. Use at your own risk. Do what you want with it, but don't DoS Qobuz, OK?
