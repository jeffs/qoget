# Tasks: Bandcamp Support & Unified CLI

**Input**: Design documents from `/specs/002-bandcamp-unified-cli/`
**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/bandcamp-api.md

**Organization**: Tasks are grouped by user story. US1 is the MVP. US2 and US3 are sequential (US2 depends on US1, US3 depends on US2).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story (US1, US2, US3)
- Exact file paths included in descriptions

---

## Phase 1: Setup

**Purpose**: Add new dependencies

- [x] T001 Add `zip` crate dependency to Cargo.toml

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared type and module changes that all user stories depend on

- [x] T002 [P] Add `Service` enum (`Qobuz`, `Bandcamp`) and add `file_extension: &'static str` field to `DownloadTask` in src/models.rs. Update all existing `DownloadTask` construction sites to pass `".mp3"`. Add Bandcamp-specific API response types: `BandcampCollectionResponse`, `BandcampCollectionItem`, `BandcampDownloadInfo`, `BandcampDownloadFormat` (per data-model.md and contracts/bandcamp-api.md)
- [x] T003 [P] Parameterize file extension in `track_path()` in src/path.rs: add `ext: &str` parameter (currently hardcoded `.mp3`). Update all call sites. Update tests in tests/path_test.rs to cover both `.mp3` and `.m4a` extensions
- [x] T004 [P] Refactor src/config.rs for multi-service config: add `[qobuz]`/`[bandcamp]` TOML section support with backward-compatible bare key fallback for Qobuz (per research.md section 3). Add `BANDCAMP_IDENTITY` env var support. Return a config struct that indicates which services are configured. Add config parsing tests in tests/config_test.rs covering: new format, old format, mixed, env var precedence, Bandcamp-only, Qobuz-only, both
- [x] T005 Update `collect_tasks()` and `build_sync_plan()` in src/sync.rs to pass file extension through from `DownloadTask` (uses the `file_extension` field added in T002). Ensure `scan_existing()` checks for the correct extension when matching files
- [x] T006 Update `download_one()` in src/download.rs to use `task.file_extension` for temp file naming (currently hardcoded `mp3.tmp`). Ensure `execute_downloads` type signature is compatible with future per-service dispatch (T016)
- [x] T007 Add `pub mod bandcamp;` to src/lib.rs

**Checkpoint**: All existing tests pass. Qobuz sync works identically to before. Foundation ready for Bandcamp integration.

---

## Phase 3: User Story 1 - Sync Bandcamp Purchases (Priority: P1)

**Goal**: A user can sync their entire Bandcamp purchase library to a local directory as AAC (`.m4a`) files, organized by artist/album.

**Independent Test**: Run `qoget sync --service bandcamp <dir>` against a Bandcamp account with purchases. Verify all purchased tracks appear as `.m4a` files in `Artist/Album/NN - Title.m4a` structure. Incremental re-run downloads nothing.

### Implementation for User Story 1

- [x] T008 [US1] Create `BandcampClient` struct and constructor in src/bandcamp.rs: takes HTTP client + identity cookie string, sets up cookie jar with `identity` cookie on `bandcamp.com` domain, sets realistic User-Agent header. Implement `verify_auth()` via `GET /api/fan/2/collection_summary` to extract and return `fan_id` (per contracts/bandcamp-api.md section 1). Return clear error on 401/403
- [x] T009 [US1] Implement `get_purchases()` in src/bandcamp.rs: paginated `POST /api/fancollection/1/collection_items` with `fan_id`, `older_than_token`, `count=100`. Loop until `items` is empty. Also fetch hidden items via `POST /api/fancollection/1/hidden_items`. Collect all `redownload_urls` mapped to their `BandcampCollectionItem`. Return combined list (per contracts/bandcamp-api.md section 2-3)
- [x] T010 [US1] Implement `get_download_info()` in src/bandcamp.rs: given a redownload_url, `GET` the download page HTML, parse `<div id="pagedata" data-blob="...">` (HTML-entity-decode the attribute value), extract `digital_items[0]`, return `BandcampDownloadInfo` with the `aac-hi` download URL (per contracts/bandcamp-api.md section 4). Error if `aac-hi` format unavailable
- [x] T011 [US1] Implement album ZIP download and extraction in src/bandcamp.rs: given a download URL (from `aac-hi` entry), stream response to temp `.zip` file, open with `zip` crate, iterate entries to extract `.m4a` files. Parse track number and title from ZIP entry filenames (pattern: `NN TrackTitle.m4a`). Return list of extracted tracks with metadata (track number, title, temp file path). Clean up ZIP after extraction. Handle single-track purchases (bare `.m4a` response, no ZIP) separately
- [x] T012 [US1] Implement `to_purchase_list()` in src/bandcamp.rs: convert `BandcampCollectionItem` list to `PurchaseList` (Vec of `Album`/`Track` using existing model types). Map `band_name` → `Artist.name`, `item_title` → `Album.title`, set `media_count=1`, generate `AlbumId` from Bandcamp item_id, generate `TrackId` from Bandcamp item_id (per research.md section 5 metadata mapping)
- [x] T013 [US1] Add rate limiting to `BandcampClient` in src/bandcamp.rs: implement token-bucket rate limiter (3 req/s burst), add 10-second backoff on HTTP 429 responses, integrate with existing retry logic pattern from src/client.rs (exponential backoff on 500/502/503/504, max 3 retries)
- [x] T014 [US1] Implement Bandcamp download dispatch in src/download.rs: add `execute_bandcamp_downloads()` function that takes `BandcampClient` + `BandcampPurchases` + target_dir + dry_run, iterates purchase items, calls `get_download_info()` then `download_and_extract()` per item, places extracted files at computed target paths, shows progress bars (indicatif). Atomic temp-file-then-rename pattern. Album-level incremental sync check
- [x] T015 [US1] Wire up Bandcamp sync flow in src/main.rs: after config loading, if Bandcamp is configured, create `BandcampClient`, call `verify_auth()`, `get_purchases()`, `to_purchase_list()`, feed into existing `collect_tasks()` → `scan_existing()` → `build_sync_plan()` pipeline, then `execute_bandcamp_downloads()`. For now, run Bandcamp only (multi-service orchestration is US2)
- [x] T016 [P] [US1] Add Bandcamp response parsing tests in tests/bandcamp_test.rs: test `BandcampCollectionResponse` JSON deserialization (sample from contracts/bandcamp-api.md), test `BandcampDownloadInfo` parsing from sample `digital_items` JSON, test ZIP filename parsing (track number + title extraction), test `to_purchase_list()` mapping

**Checkpoint**: `qoget sync <dir>` works for Bandcamp-only configuration. Downloads AAC files, organizes correctly, incremental sync skips existing. Qobuz-only users see no change.

---

## Phase 4: User Story 2 - Unified Multi-Service Sync (Priority: P2)

**Goal**: A user with both services configured can sync both libraries with a single `qoget sync <dir>` command.

**Independent Test**: Configure both Qobuz and Bandcamp, run `qoget sync <dir>`, verify tracks from both services appear. Re-run downloads nothing. Both `.mp3` (Qobuz) and `.m4a` (Bandcamp) files coexist.

### Implementation for User Story 2

- [x] T017 [US2] Implement multi-service orchestration in src/main.rs: iterate all configured services from config, create appropriate client per service, fetch purchases and build download tasks per service (with correct file_extension), merge all tasks into a single `Vec<DownloadTask>`, feed into unified `build_sync_plan()`. Execute downloads per-service (Qobuz tasks → `execute_downloads()`, Bandcamp tasks → `execute_bandcamp_downloads()`)
- [x] T018 [US2] Implement per-service error isolation in src/main.rs: wrap each service's auth+fetch+download in error handling, if one service fails (auth error, network error), log the failure and continue with remaining services. Aggregate `SyncResult` across services. Exit nonzero if any service had failures
- [x] T019 [US2] Implement per-service progress display in src/main.rs: print service name header before each service's sync phase (e.g., `Syncing Qobuz...`, `Syncing Bandcamp...`). Per-service summary in final output: `Qobuz: 5 downloaded, 100 skipped. Bandcamp: 12 downloaded, 50 skipped.`

**Checkpoint**: Single `qoget sync <dir>` syncs both services. Service failure is isolated. Per-service progress visible.

---

## Phase 5: User Story 3 - Selective Service Sync (Priority: P3)

**Goal**: User can run `qoget sync --service bandcamp <dir>` to sync only one service.

**Independent Test**: Configure both services, run with `--service bandcamp`, verify only Bandcamp tracks downloaded. Run with `--service qobuz`, verify only Qobuz. Run with `--service invalid`, verify clear error.

### Implementation for User Story 3

- [x] T020 [US3] Add `--service` CLI flag to clap argument parsing in src/main.rs: optional string argument accepting `qobuz` or `bandcamp` (case-insensitive). Map to `Option<Service>` enum. Pass to orchestration logic from T017
- [x] T021 [US3] Implement service filter logic in src/main.rs: if `--service` specified, filter configured services to only the requested one. If the requested service is not configured, report clear error listing how to configure it (config file path + env var name). If no `--service` specified, behavior unchanged (sync all configured)
- [x] T022 [US3] Validate backward compat: ensure `--service qobuz` with only Qobuz configured produces identical output to the pre-feature behavior (same files, same progress output, same exit code). No `--service` flag with only Qobuz configured also identical

**Checkpoint**: `--service` flag works. Qobuz-only users with no flag see zero behavioral changes (FR-012).

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Edge cases, documentation, final validation

- [x] T023 Handle edge case: no services configured — print helpful error listing supported services and how to configure each (config file example + env vars) in src/main.rs
- [x] T024 Handle edge case: Bandcamp purchase with no `aac-hi` format available — report track as unavailable, continue with remaining tracks in src/bandcamp.rs
- [x] T025 [P] Ensure all existing tests pass: run `cargo test`, fix any regressions in tests/path_test.rs, tests/models_test.rs, tests/signature_test.rs
- [x] T026 Validate `--dry-run` works for Bandcamp and multi-service sync: run `qoget sync --dry-run <dir>` with Bandcamp configured, verify planned downloads are listed without downloading. Verify with both services configured. If sync pipeline doesn't already handle this, wire it through in src/main.rs
- [x] T027 Update README.md with multi-service configuration, Bandcamp cookie setup instructions, `--service` flag usage

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 — BLOCKS all user stories
- **US1 (Phase 3)**: Depends on Phase 2 — the MVP
- **US2 (Phase 4)**: Depends on US1 (Phase 3) — extends single-service to multi-service
- **US3 (Phase 5)**: Depends on US2 (Phase 4) — adds filtering to multi-service
- **Polish (Phase 6)**: Can start after US1, ideally after US3

### User Story Dependencies

```
Phase 1 (Setup) → Phase 2 (Foundational) → Phase 3 (US1: Bandcamp sync)
                                                    ↓
                                             Phase 4 (US2: Multi-service)
                                                    ↓
                                             Phase 5 (US3: --service flag)
                                                    ↓
                                             Phase 6 (Polish)
```

US2 builds on US1 (needs Bandcamp client to exist). US3 builds on US2 (needs multi-service loop to filter).

### Within Each Phase

- Tasks without `[P]` are sequential (each builds on the previous)
- Tasks with `[P]` can run in parallel with other `[P]` tasks in the same phase
- Within US1: T008→T009→T010→T011→T012 are sequential (each builds on the previous in bandcamp.rs). T013 (rate limiting) can be done after T008. T014 depends on T011. T015 depends on T014. T016 is parallel (test file)

### Parallel Opportunities

**Phase 2** (all [P], different files):
```
T002 (models.rs) ‖ T003 (path.rs) ‖ T004 (config.rs)
     → T005 (sync.rs) ‖ T006 (download.rs) ‖ T007 (lib.rs)
```

**Phase 3** (within US1):
```
T008→T009→T010→T011→T012→T013 (bandcamp.rs, sequential)
T014 (download.rs, after T011)
T015 (main.rs, after T014)
T016 (bandcamp_test.rs, parallel with implementation)
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001)
2. Complete Phase 2: Foundational (T002-T007)
3. Complete Phase 3: User Story 1 (T008-T016)
4. **STOP and VALIDATE**: Test Bandcamp sync independently
5. Bandcamp sync is fully functional at this point

### Incremental Delivery

1. Setup + Foundational → foundation ready
2. Add US1 → Bandcamp sync works → validate (MVP)
3. Add US2 → both services sync in one command → validate
4. Add US3 → `--service` filter works → validate
5. Polish → edge cases, docs

---

## Notes

- All Bandcamp client code goes in a single new file: src/bandcamp.rs
- No service trait — enum dispatch in main.rs (per plan.md design decision)
- Bandcamp albums download as ZIPs; single tracks download as bare files
- File extension (`.mp3` vs `.m4a`) flows through the pipeline via `DownloadTask.file_extension`
- Rate limiting is critical for Bandcamp (3 req/s max, 10s backoff on 429)
- Existing Qobuz behavior must be 100% preserved (FR-012)
