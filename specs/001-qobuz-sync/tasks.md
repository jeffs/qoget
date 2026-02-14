# Tasks: Qobuz Purchase Sync CLI

**Input**: Design documents from `/specs/001-qobuz-sync/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/qobuz-api.md

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup

**Purpose**: Initialize the Rust project with all dependencies and module structure.

- [x] T001 Create Cargo.toml with edition = "2024", rust-version = "1.93", and dependencies: tokio (features: rt-multi-thread, macros, fs), reqwest (features: json, stream), serde (features: derive), serde_json, clap (features: derive), toml, indicatif, anyhow, futures, md5, base64, regex. Create src/main.rs declaring modules: config, models, path, bundle, client, sync, download. Create empty module files for each: src/config.rs, src/models.rs, src/path.rs, src/bundle.rs, src/client.rs, src/sync.rs, src/download.rs

---

## Phase 2: Foundational (Shared Types and Utilities)

**Purpose**: Pure types and utilities that ALL user stories depend on. No network I/O in this phase.

- [x] T002 [P] Implement domain types and serde models in src/models.rs. Newtype wrappers: TrackId(u64), AlbumId(String), TrackNumber(u8), DiscNumber(u8) — each with Display, serde Deserialize (deserialize from raw JSON values). Serde structs matching the Qobuz API (see contracts/qobuz-api.md): Artist { id: u64, name: String }, Album { id/qobuz_id, title, version: Option, artist: Artist, media_count: u8, tracks_count: u16, tracks: Option<PaginatedList<Track>> }, Track { id: TrackId, title, track_number: TrackNumber, media_number: DiscNumber, duration: u32, performer: Artist, isrc: Option<String> }, PaginatedList<T> { offset: u64, limit: u64, total: u64, items: Vec<T> }, PurchaseResponse { albums: PaginatedList<Album>, tracks: PaginatedList<Track> }, LoginResponse { user_auth_token: String, user: UserInfo }, UserInfo { id: u64 }, FileUrlResponse { track_id: u64, url: String, format_id: u8, mime_type: String }. Aggregated domain type: PurchaseList { albums: Vec<Album>, tracks: Vec<Track> } (all pages combined; constructed by client.rs from paginated PurchaseResponse pages). Other domain types: AppCredentials { app_id: String, app_secret: String }, UserAuth { token: String, user_id: u64 }, Session { credentials: AppCredentials, auth: UserAuth }, DownloadTask { track: Track, album: Album, target_path: PathBuf }, SkipReason enum { AlreadyExists, DryRun }, SkippedTrack { track: Track, target_path: PathBuf, reason: SkipReason }, SyncPlan { downloads: Vec<DownloadTask>, skipped: Vec<SkippedTrack>, total_tracks: usize }, DownloadError { task: DownloadTask, error: String }, SyncResult { succeeded: Vec<DownloadTask>, failed: Vec<DownloadError>, skipped: Vec<SkippedTrack> }
- [x] T003 [P] Implement path construction and sanitization in src/path.rs. Function sanitize_component(s: &str) -> String that replaces / and \ with -, replaces : with -, removes * ? " < > |, trims whitespace, removes leading dots, collapses consecutive spaces, and truncates to 255 bytes. Function track_path(base: &Path, album: &Album, track: &Track) -> PathBuf that builds: base/album_artist/album_title[/Disc N]/NN - [Track Artist - ]Title.mp3. Rules: add "Disc N/" subdirectory only when album.media_count > 1; include track artist in filename only when track.performer.name != album.artist.name; zero-pad track number to 2 digits. All components passed through sanitize_component
- [x] T004 [P] Implement config loading in src/config.rs. Struct Config { username: String, password: String, app_id: Option<String>, app_secret: Option<String> }. Struct FileConfig with serde Deserialize for TOML: same fields but all Option. Function load_config() -> Result<Config> that: reads ~/.config/qoget/config.toml if it exists (using dirs or XDG manually), overlays QOBUZ_USERNAME and QOBUZ_PASSWORD env vars (env takes precedence), returns error if username or password missing from both sources. app_id and app_secret are optional overrides

**Checkpoint**: Foundation ready — all pure types and utilities available for API integration

---

## Phase 3: User Story 1 - First-Time Full Sync (Priority: P1) — MVP

**Goal**: Authenticate with Qobuz, fetch all purchases, download every track as MP3 320 into Artist/Album/Track directory structure.

**Independent Test**: Run `qoget sync ~/Music/Qobuz` against a Qobuz account with purchases. Verify all tracks appear as playable MP3 files in the correct directory hierarchy.

### Implementation for User Story 1

- [x] T005 [P] [US1] Implement bundle.js credential extraction in src/bundle.rs. Function extract_credentials(http_client: &reqwest::Client) -> Result<AppCredentials>. Steps per contracts/qobuz-api.md "App Credential Extraction": (1) GET https://play.qobuz.com/login, extract bundle.js URL via regex on the HTML, (2) GET the bundle.js, (3) extract app_id via regex `production:\{api:\{appId:"(\d{9})"`, (4) extract seed/timezone pairs via regex `initialSeed\("([\w=]+)",window\.utimezone\.([\w]+)\)`, (5) for each timezone find info/extras via regex, (6) concatenate seed+info+extras, strip last 44 chars, base64-decode to get candidate secrets, (7) validate each candidate by calling GET /track/getFileUrl for a known track (e.g., track_id=19512574, format_id=27, intent=stream) — HTTP 200 or 401 means valid secret, HTTP 400 means invalid, try next candidate, (8) return AppCredentials with app_id and the validated secret. Error if no candidate validates. Accept reqwest::Client as parameter (don't construct internally)
- [x] T006 [P] [US1] Implement Qobuz API client in src/client.rs. Struct QobuzClient wrapping reqwest::Client + Session. Function login(client: &reqwest::Client, app_id: &str, username: &str, password: &str) -> Result<UserAuth>: GET /user/login with email=username, password=MD5(password), app_id; parse LoginResponse; return UserAuth { token, user_id }. Method get_purchases(&self) -> Result<PurchaseList>: paginate GET /purchase/getUserPurchases with limit=500, aggregating all album and track items across pages. Method get_album(&self, album_id: &str) -> Result<Album>: GET /album/get?album_id=X, returns album with embedded tracks. Method get_file_url(&self, track_id: TrackId, format_id: u8) -> Result<String>: GET /track/getFileUrl with signed request per contracts/qobuz-api.md — signature = MD5("trackgetFileUrlformat_id{fid}intentstreamtrack_id{tid}{ts}{secret}"), note "intentstream" is hardcoded in signature regardless of actual intent parameter. All methods send X-App-Id and X-User-Auth-Token headers. Helper function generate_request_sig(track_id, format_id, timestamp, app_secret) -> String
- [x] T007 [US1] Implement sync planning in src/sync.rs. Function build_sync_plan(purchases: &PurchaseList, base_dir: &Path) -> SyncPlan. For each purchased album and its tracks: compute target_path using path::track_path, create DownloadTask { track, album, target_path }. Deduplicate by TrackId: if the same track appears in multiple purchases (e.g., as a standalone single and within an album), keep only the album version (prefer the DownloadTask whose album has more than one track). Add all deduplicated tasks to SyncPlan.downloads. Set total_tracks to the deduplicated count. This is a pure function with no I/O
- [x] T008 [US1] Implement download execution in src/download.rs. Async function execute_downloads(client: &QobuzClient, plan: SyncPlan) -> Result<SyncResult>. For each DownloadTask: call client.get_file_url(track.id, 5) to get MP3 320 URL, then HTTP GET that URL streaming the body to a temp file in the same directory, then rename temp to target_path on success. Use futures::stream::iter + buffer_unordered(4) for bounded parallelism. Create parent directories with tokio::fs::create_dir_all. On failure: delete temp file, record DownloadError. Return SyncResult with succeeded/failed/skipped lists
- [x] T009 [US1] Wire CLI entry point in src/main.rs. Use clap derive: subcommand Sync with positional arg target_dir: PathBuf and optional --dry-run flag. Main flow: (1) load_config(), (2) if config has app_id+app_secret use those, else call bundle::extract_credentials(), (3) call client::login(), (4) construct QobuzClient with Session, (5) call client.get_purchases(), (6) for each album in purchases call client.get_album() to populate tracks, (7) call sync::build_sync_plan(), (8) call download::execute_downloads(), (9) print summary and exit. Wire tokio runtime with #[tokio::main]

### Tests for User Story 1

- [x] T010 [P] [US1] Unit tests for request signature generation in tests/signature_test.rs. Test generate_request_sig with known inputs: track_id=216020864, format_id=5, a fixed timestamp, and a test secret. Verify output matches precomputed MD5 hex digest. Test that signature always contains "intentstream" regardless of actual download intent
- [x] T011 [P] [US1] Unit tests for path construction in tests/path_test.rs. Test cases: (1) single-disc album → Artist/Album/01 - Title.mp3, (2) multi-disc album (media_count=2) → Artist/Album/Disc 1/01 - Title.mp3, (3) compilation (track performer != album artist) → Various Artists/Album/01 - Track Artist - Title.mp3, (4) sanitization: title with / : * ? characters → replaced/removed correctly, (5) leading dot removal, (6) consecutive space collapse, (7) 255-byte truncation
- [x] T012 [P] [US1] Unit tests for model deserialization in tests/models_test.rs. Embed JSON fixtures matching the response shapes in contracts/qobuz-api.md. Test: LoginResponse parses user_auth_token and user.id. PurchaseResponse parses albums.items and tracks.items with correct totals. Album with embedded tracks parses track_number, media_number, performer. FileUrlResponse parses url field. Verify TrackId, AlbumId newtypes deserialize from raw values

**Checkpoint**: MVP complete — full sync works end-to-end. Test by running against a real Qobuz account.

---

## Phase 4: User Story 2 - Incremental Sync (Priority: P2)

**Goal**: Skip tracks that already exist locally as non-empty files. Re-download missing or empty files. Support --dry-run to preview without downloading.

**Independent Test**: Run sync twice. Second run should report "up to date" and download nothing. Delete one file, run again — only that file re-downloads.

### Implementation for User Story 2

- [x] T013 [US2] Add incremental skip logic to src/sync.rs, keeping sync planning pure. Add a new struct ExistingFiles (a HashSet<PathBuf> of local files that exist and are non-empty). Add a standalone async function scan_existing(base_dir: &Path, plan: &SyncPlan) -> ExistingFiles that walks the target paths in the plan and stats each one. Modify build_sync_plan signature to build_sync_plan(purchases: &PurchaseList, base_dir: &Path, existing: &ExistingFiles, dry_run: bool) -> SyncPlan: for each track, if target_path is in existing, add to skipped with SkipReason::AlreadyExists; if dry_run is true and not already skipped, add remaining to skipped with SkipReason::DryRun; otherwise add to downloads. build_sync_plan remains a pure function — all I/O is in scan_existing. In src/main.rs, call scan_existing before build_sync_plan
- [x] T014 [US2] Implement --dry-run output in src/main.rs. When --dry-run is set, after building sync plan: print each DownloadTask's target_path (one per line), print summary of "N tracks to download, M already synced", and exit without downloading

**Checkpoint**: Incremental sync works. Running twice downloads nothing the second time.

---

## Phase 5: User Story 3 - Progress and Error Visibility (Priority: P3)

**Goal**: Show progress bars during download, continue on individual failures, display a summary with successes/failures/skips, exit nonzero if any failures.

**Independent Test**: Run sync against a library with multiple albums. Observe progress bars updating. Simulate a failure (e.g., disconnect network mid-sync) and verify the tool reports failures and continues with remaining tracks.

### Implementation for User Story 3

- [x] T015 [US3] Add indicatif progress bars to src/download.rs. Create a MultiProgress with an overall ProgressBar (total = number of DownloadTasks, incremented per completed download) and a per-download ProgressBar showing bytes downloaded for the current file. Overall bar message format: "[N/Total] Downloading: Artist - Album - Track". Clear per-download bars on completion. Use indicatif::ProgressStyle for a clean format
- [x] T016 [US3] Add error collection, summary output, and exit code to src/main.rs. After execute_downloads returns SyncResult: print summary section listing count of succeeded, failed, skipped. For each failed download: print track name and error message. Exit with code 0 if failed.is_empty(), code 1 otherwise (FR-014). Print "Library is up to date" when both downloads and failed are empty
- [x] T017 [US3] Add partial file cleanup on download failure in src/download.rs. When a download fails (network error, write error, etc.), delete the temp file if it exists before recording the DownloadError. Use a Drop guard or explicit cleanup in the error path to ensure temp files never persist (FR-012)

**Checkpoint**: All three user stories complete. Tool shows progress, handles errors gracefully, and reports results clearly.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Edge cases and robustness improvements that span multiple user stories.

- [x] T018 Add retry with exponential backoff for transient failures in src/client.rs. Retry on HTTP 429, 500, 502, 503, 504 with backoff: 1s, 2s, 4s, max 3 retries. Apply to all API calls (login, purchases, album/get, getFileUrl) and to the download HTTP GET. Do not retry on 401 (auth failure) or 400 (bad request)
- [x] T019 Validate end-to-end by running quickstart.md scenarios: full sync to a test directory, incremental re-sync, and dry-run. Verify directory structure matches data-model.md filesystem layout

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — start immediately
- **Foundational (Phase 2)**: Depends on Phase 1 — BLOCKS all user stories
- **US1 (Phase 3)**: Depends on Phase 2 — this is the MVP
- **US2 (Phase 4)**: Depends on US1 (modifies sync.rs and main.rs from Phase 3)
- **US3 (Phase 5)**: Depends on US1 (modifies download.rs and main.rs from Phase 3)
- **Polish (Phase 6)**: Depends on all user stories being complete

### User Story Dependencies

- **US1 (P1)**: Requires foundational types only. Core pipeline: config → bundle → login → purchases → sync plan → download
- **US2 (P2)**: Requires US1's sync.rs and main.rs to exist. Adds skip logic and dry-run
- **US3 (P3)**: Requires US1's download.rs and main.rs to exist. Adds progress, error handling, cleanup

### Within Each Phase

- Tasks marked [P] can run in parallel (different files, no shared state)
- Unmarked tasks depend on prior tasks in the same phase completing
- Tests (T010-T012) can run in parallel with each other and after their corresponding implementation tasks

### Parallel Opportunities

**Phase 2** (all [P]):
```
T002 (models.rs) ‖ T003 (path.rs) ‖ T004 (config.rs)
```

**Phase 3** (bundle + client are [P], then sequential):
```
T005 (bundle.rs) ‖ T006 (client.rs)
    ↓
T007 (sync.rs) → T008 (download.rs) → T009 (main.rs)
    ↓ (parallel with implementation)
T010 (sig tests) ‖ T011 (path tests) ‖ T012 (model tests)
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001)
2. Complete Phase 2: Foundational (T002-T004)
3. Complete Phase 3: User Story 1 (T005-T012)
4. **STOP and VALIDATE**: Run `qoget sync ~/Music/Qobuz` against a real account
5. Verify all purchased tracks appear as MP3 files in correct directory structure

### Incremental Delivery

1. Setup + Foundational → Types and utilities ready
2. Add US1 → Full sync works → MVP usable
3. Add US2 → Incremental sync + dry-run → Daily-driver usable
4. Add US3 → Progress bars + error handling → Production-quality
5. Add Polish → Retry/backoff → Robust

---

## Notes

- [P] tasks = different files, no dependencies on each other
- [Story] label maps task to specific user story for traceability
- US2 and US3 both modify files created in US1, so they must follow US1 sequentially
- The bundle.js extraction (T005) is the most fragile component — config file overrides provide a fallback
- All API endpoint details are in contracts/qobuz-api.md; all type definitions are in data-model.md
