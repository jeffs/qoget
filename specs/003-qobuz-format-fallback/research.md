# Research: Qobuz Format Fallback

## 1. Qobuz Format IDs

**Decision**: Use `format_id=5` for MP3 320 (existing) and `format_id=6` for CD Quality FLAC as fallback.

**Rationale**: These are the well-established Qobuz format IDs used by all known third-party tools (qobuz-dl, streamrip, etc.). The existing codebase already uses `format_id=5` for MP3 320 and `format_id=27` for bundle validation (hi-res). Format ID 6 (CD Quality FLAC, 16-bit/44.1kHz) is the lossless tier immediately above MP3.

**Alternatives considered**:
- Format ID 7 (24-bit up to 96kHz): Higher quality but larger files and not universally available. Out of scope per spec.
- Format ID 27 (24-bit up to 192kHz): Already used in bundle.rs for credential validation. Too large for a default fallback.

## 2. Error Response When Format Unavailable

**Decision**: Treat any `Err` from `get_file_url` with MP3 format as a trigger for CD Quality fallback.

**Rationale**: Qobuz's API is undocumented. The `send_with_retry` function in `client.rs` already retries on transient errors (429, 500, 502, 503, 504) and only surfaces non-retryable errors (400, 403, 404, etc.). By the time `download_one` sees an `Err`, transient issues have been exhausted. The remaining failure modes are:
- 400: Format not available for this track
- 403: Not purchased / region-locked
- 404: Track doesn't exist

All of these are worth retrying with CD Quality — if the track is genuinely inaccessible (not purchased), the CD Quality request will fail too with the same or similar error.

**Alternatives considered**:
- Parse specific HTTP status codes or error messages: Fragile (undocumented API), and false positives are harmless (trying CD Quality on a non-purchased track just gives another error).
- Only retry on 400: Too narrow — we don't know all the error codes Qobuz uses for "format unavailable."

## 3. Incremental Sync with Mixed Extensions

**Decision**: Check both `.mp3` and `.flac` extensions in `scan_existing()`.

**Rationale**: The sync pipeline builds `DownloadTask` with `.mp3` as the planned extension. But after a fallback download, the file on disk is `.flac`. The next sync run needs to recognize this. The simplest approach: for each task's target path, also check the path with `.flac` substituted.

**Alternatives considered**:
- Scan the directory for any file matching the track pattern regardless of extension: More complex, requires glob matching, and slower for large libraries.
- Store a manifest of downloaded files: Adds state management complexity. The filesystem is already the source of truth.
- Change `collect_tasks` to emit both `.mp3` and `.flac` tasks: Doubles the task list size and complicates deduplication.

## 4. Return Value from download_one

**Decision**: Change `download_one` return type from `Result<()>` to `Result<DownloadOutcome>` where `DownloadOutcome` indicates whether the track was downloaded as MP3 or via FLAC fallback.

**Rationale**: The caller (`execute_downloads`) needs to count fallback downloads for the summary. A simple enum return is the minimal change to communicate this.

**Alternatives considered**:
- Atomic counter (Arc<AtomicUsize>): Works for counting but less composable and harder to test.
- Modify `DownloadTask` to track actual format: Mutating the task conflates planning with execution.
