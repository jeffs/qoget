# Implementation Plan: Qobuz Purchase Sync CLI

**Branch**: `001-qobuz-sync` | **Date**: 2026-02-14 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/001-qobuz-sync/spec.md`

## Summary

Build a Rust CLI (`qoget`) that authenticates with the Qobuz API, retrieves the user's purchased albums and tracks, and downloads them as MP3 320 files into an organized local directory structure. The tool supports incremental sync (skip existing files by size), bounded parallel downloads (~4 concurrent), multi-disc albums, compilation handling, dry-run mode, and config file + env var credential management. The Qobuz API is undocumented; authentication and download URL signing require extracting app credentials from the Qobuz web player's JavaScript bundle.

## Technical Context

**Language/Version**: Rust 2024 edition, MSRV 1.70+
**Primary Dependencies**: tokio 1.47, reqwest 0.12, serde/serde_json 1.0, clap 4.5, toml 0.8, indicatif 0.17, anyhow 1.0, futures 0.3, md5
**Storage**: Local filesystem only (no database; sync state derived from file presence + size)
**Testing**: `cargo test` (unit tests for path construction, model deserialization, signature generation)
**Target Platform**: macOS / Linux CLI (single binary)
**Project Type**: Single Rust binary crate
**Performance Goals**: 4 concurrent downloads; incremental sync <30s when up-to-date
**Constraints**: No obscure crates; direct Qobuz HTTP API calls; MP3 320 only
**Scale/Scope**: Personal tool; library sizes from tens to hundreds of albums

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

No project-specific constitution has been defined (template only). Gate passes trivially. The design follows principles from CLAUDE.md:
- Functional style: pure functions for path construction, signature generation, sync planning
- Parse, don't validate: newtype wrappers for TrackId, AlbumId, DiscNumber, TrackNumber
- Make invalid states unrepresentable: `Session` requires valid auth token; `SyncPlan` separates downloads from skips at the type level
- Small, composable pieces: each module is orthogonal (client, models, paths, sync logic, config, bundle extraction)

## Project Structure

### Documentation (this feature)

```text
specs/001-qobuz-sync/
├── plan.md              # This file
├── research.md          # Qobuz API research (Phase 0)
├── data-model.md        # Domain types and API models (Phase 1)
├── quickstart.md        # Developer setup guide (Phase 1)
├── contracts/           # Qobuz API contract documentation (Phase 1)
│   └── qobuz-api.md
└── tasks.md             # Implementation tasks (Phase 2 - /speckit.tasks)
```

### Source Code (repository root)

```text
src/
├── main.rs              # CLI entry point: clap parsing, config loading, orchestration
├── config.rs            # Config file (~/.config/qoget/config.toml) + env var loading
├── client.rs            # Qobuz API client: auth, purchases, download URLs, signing
├── bundle.rs            # App ID/secret extraction from play.qobuz.com bundle.js
├── models.rs            # Serde types for Qobuz API responses (albums, tracks, purchases)
├── sync.rs              # Sync planning: diff remote vs local, produce SyncPlan
├── download.rs          # Bounded parallel download execution with progress
└── path.rs              # Filesystem path construction, sanitization, naming rules

tests/
├── models_test.rs       # Deserialization of captured API response fixtures
├── path_test.rs         # Path sanitization, multi-disc, compilation naming
└── signature_test.rs    # Request signature generation against known values
```

**Structure Decision**: Single binary crate. No workspace, no library split. The crate is small enough that a flat `src/` with 8 focused modules provides clear separation without over-engineering. Each module has a single responsibility and exposes a small public API.

## Complexity Tracking

No constitution violations to justify.
