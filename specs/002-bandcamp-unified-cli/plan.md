# Implementation Plan: Bandcamp Support & Unified CLI

**Branch**: `002-bandcamp-unified-cli` | **Date**: 2026-02-14 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/002-bandcamp-unified-cli/spec.md`

## Summary

Add Bandcamp purchase syncing to the existing Qobuz sync tool, behind a unified CLI. Bandcamp uses cookie-based authentication (the `identity` cookie) and delivers albums as ZIP files containing AAC (`.m4a`) tracks. The tool will sync all configured services by default, with an optional `--service` filter. Existing Qobuz-only behavior is fully preserved.

Key technical challenges: (1) Bandcamp has no public API — we reverse-engineer the same cookie-based web endpoints used by existing tools; (2) albums download as ZIPs requiring extraction; (3) config format must evolve to support per-service credentials without breaking existing flat configs.

## Technical Context

**Language/Version**: Rust (edition 2021, same as existing project)
**Primary Dependencies**: reqwest (HTTP), serde/serde_json (JSON), toml (config), clap (CLI), zip (new — ZIP extraction), indicatif (progress), futures (async parallelism), regex (HTML parsing)
**Storage**: Local filesystem (same as existing)
**Testing**: cargo test (existing test suite + new tests)
**Target Platform**: macOS, Linux (same as existing)
**Project Type**: Single binary CLI
**Performance Goals**: Incremental sync <30s when no new purchases (per spec SC-003). Bounded parallelism: 4 concurrent downloads (Qobuz), rate-limited 3 req/s (Bandcamp).
**Constraints**: Bandcamp rate limit ~3 req/s; album ZIPs can be 50-200MB; identity cookie is opaque and long-lived.
**Scale/Scope**: Typical collection 50-500 albums per service; single user CLI tool.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Constitution file is an unfilled template — no project-specific gates defined. Gate passes by default.

**Post-Phase 1 re-check**: Design maintains single-binary, single-project structure. No new architectural patterns beyond a service enum and a new client module. Functional composition preserved. Types encode valid states. Gate passes.

## Project Structure

### Documentation (this feature)

```text
specs/002-bandcamp-unified-cli/
├── plan.md              # This file
├── research.md          # Phase 0: Bandcamp API research
├── data-model.md        # Phase 1: Entity model
├── quickstart.md        # Phase 1: Getting started guide
├── contracts/
│   └── bandcamp-api.md  # Phase 1: Bandcamp endpoint contracts
└── tasks.md             # Phase 2 output (/speckit.tasks)
```

### Source Code (repository root)

```text
src/
├── main.rs              # MODIFY: multi-service orchestration, --service flag
├── lib.rs               # MODIFY: export new modules
├── models.rs            # MODIFY: add Service enum, file_extension to DownloadTask
├── config.rs            # MODIFY: multi-service config with backward compat
├── client.rs            # EXISTING: Qobuz client (minimal changes)
├── bandcamp.rs          # NEW: Bandcamp client (auth, collection, download)
├── sync.rs              # MODIFY: accept file_extension in path computation
├── download.rs          # MODIFY: support ZIP extraction for Bandcamp downloads
├── path.rs              # MODIFY: parameterize file extension
└── bundle.rs            # EXISTING: unchanged

tests/
├── path_test.rs         # MODIFY: test .m4a extension
├── models_test.rs       # MODIFY: add Bandcamp response parsing tests
├── signature_test.rs    # EXISTING: unchanged
├── bandcamp_test.rs     # NEW: Bandcamp API response parsing, ZIP extraction
└── config_test.rs       # NEW: multi-service config parsing, backward compat
```

**Structure Decision**: Single project, flat module layout — matches existing structure. New Bandcamp client is a peer module to `client.rs`, not a nested module hierarchy. This keeps the codebase simple and avoids unnecessary abstraction layers.

## Design Decisions

### No Service Trait

The Qobuz and Bandcamp download flows are fundamentally different (individual file URLs vs ZIP downloads). Forcing them behind a common async trait would require either:
- Leaky abstractions (trait methods that only one impl uses)
- Lowest-common-denominator interface (lose type safety)

Instead: `main.rs` orchestrates each service explicitly via enum dispatch. The sync module (`collect_tasks`, `build_sync_plan`) is already generic — it operates on `Album`/`Track` types regardless of origin. The download module dispatches per-service. This is simpler, more correct, and avoids premature abstraction.

### Bandcamp Album Downloads as ZIP

Bandcamp delivers albums as ZIP files. The download flow:
1. GET the download URL → receive ZIP stream
2. Write ZIP to temp file
3. Extract individual `.m4a` tracks
4. Rename to match naming convention (`NN - Title.m4a`)
5. Place in target directory
6. Clean up temp ZIP

This is handled in `download.rs` with a Bandcamp-specific download path.

### Config Backward Compatibility

```toml
# Old format (still works, Qobuz only):
username = "user@email.com"
password = "secret"

# New format:
[qobuz]
username = "user@email.com"
password = "secret"

[bandcamp]
identity_cookie = "6%09..."
```

Config loading: try new format first, fall back to bare keys for Qobuz. Both can coexist. Environment variables: `QOBUZ_USERNAME`/`QOBUZ_PASSWORD` (existing), `BANDCAMP_IDENTITY` (new).

### File Extension Parameterization

`track_path()` currently hardcodes `.mp3`. Change to accept file extension as parameter:
- Qobuz: `.mp3`
- Bandcamp: `.m4a`

The extension is determined at task creation time and flows through the pipeline.

## Complexity Tracking

No constitution violations to justify.
