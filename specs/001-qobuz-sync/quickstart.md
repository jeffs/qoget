# Quickstart: Qobuz Purchase Sync CLI

**Feature Branch**: `001-qobuz-sync`
**Date**: 2026-02-14

## Prerequisites

- Rust toolchain (1.93+ / 2024 edition): `rustup default stable`
- A Qobuz account with at least one purchased album or track
- Network access to `qobuz.com` and `play.qobuz.com`

## Setup

```bash
# Clone and build
git clone <repo-url> qoget
cd qoget
cargo build --release
```

## Configuration

Create `~/.config/qoget/config.toml`:

```toml
username = "your-email@example.com"
password = "your-qobuz-password"
```

Or use environment variables (these override the config file):

```bash
export QOBUZ_USERNAME="your-email@example.com"
export QOBUZ_PASSWORD="your-qobuz-password"
```

## Usage

```bash
# Full sync to a directory
qoget sync ~/Music/Qobuz

# Dry-run: see what would be downloaded
qoget sync --dry-run ~/Music/Qobuz

# Incremental sync (same command; skips existing files)
qoget sync ~/Music/Qobuz
```

## Output Structure

```
~/Music/Qobuz/
├── Pink Floyd/
│   └── The Dark Side of the Moon/
│       ├── 01 - Speak to Me.mp3
│       ├── 02 - Breathe (In the Air).mp3
│       └── ...
└── Various Artists/
    └── Jazz Classics/
        ├── 01 - Miles Davis - So What.mp3
        └── ...
```

## Running Tests

```bash
cargo test
```

## Development

Key modules:
- `src/client.rs` -- Qobuz API interactions
- `src/bundle.rs` -- App credential extraction (most fragile component)
- `src/sync.rs` -- Sync planning logic (pure, testable)
- `src/path.rs` -- Filesystem path construction (pure, testable)
- `src/models.rs` -- API response types

## Troubleshooting

**Authentication failure**: Verify credentials. Qobuz expects email (not username) for most accounts.

**Bundle extraction failure**: The app_id/secret extraction from `play.qobuz.com` may break when Qobuz updates their frontend. If this happens, you can manually set `app_id` and `app_secret` in the config file (obtain from other tools or by inspecting the web player in a browser).

**Download errors for specific tracks**: Some tracks may not be downloadable at MP3 320 quality. The tool will report these and continue with remaining tracks.
