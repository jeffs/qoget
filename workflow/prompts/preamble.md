# qoget — Project Context

qoget is a Rust CLI tool for syncing purchased music from Qobuz and Bandcamp.

## Project Structure

- `src/lib.rs` — Module exports
- `src/main.rs` — CLI entry point (clap-based subcommands)
- `src/models.rs` — Shared data types (Album, Track, Artist, newtypes)
- `src/bandcamp.rs` — Bandcamp client (auth, purchases, download, ZIP extract)
- `src/client.rs` — Qobuz HTTP client and signature generation
- `src/download.rs` — Download orchestration for both services
- `src/sync.rs` — Sync workflow
- `src/bundle.rs` — Bundle handling
- `src/config.rs` — TOML config parsing with env var override
- `src/path.rs` — File path sanitization and organization
- `tests/` — Integration tests (one file per module)

## Test Conventions

- Tests live in `tests/*_test.rs`, NOT inline in `src/`
- Tests use `#[test]` — no async test runtime, no test frameworks
- Data is constructed inline using helper functions (e.g. `make_item()`)
- JSON fixtures use `serde_json::from_str()` with raw string literals
- **NEVER** construct `reqwest::Client` in tests
- **Avoid real HTTP requests** — test data should be inline JSON/structs.
  If network access is available (no proxy), you may run `cargo run` to
  observe real API behavior for diagnosis, but keep calls to a minimum.
  Never add real API URLs to test source files.

## Hard Constraints

- **No `pub(crate)`** — use `pub` where needed
- **Wrap comments at 80 columns**
- **Use `jj` for VCS** — never use `git` commands
- **Functional style** — prefer small composable functions, avoid mutation
- **`anyhow::Result`** for error handling throughout
- **No real API URLs in test files** — no `qobuz.com`, `bandcamp.com`,
  `akamaized.net`, `popplers5`, `bcbits.com`

## Cargo

- Edition 2024, Rust 1.93+
- Key deps: tokio, reqwest, serde/serde_json, clap, anyhow, regex, zip

## Existing Test Example (for reference)

```rust
#[test]
fn aac_hi_url_missing() {
    let mut downloads = HashMap::new();
    downloads.insert(
        "mp3-320".to_string(),
        BandcampDownloadFormat {
            url: "https://example.com/mp3".to_string(),
            size_mb: "120MB".to_string(),
        },
    );
    let info = BandcampDownloadInfo {
        item_id: 1,
        title: "Test Album".to_string(),
        artist: "Test Artist".to_string(),
        download_type: "a".to_string(),
        downloads,
    };
    let err = qoget::bandcamp::aac_hi_url(&info).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("aac-hi"));
}
```
