# Feature Specification: Qobuz Purchase Sync CLI

**Feature Branch**: `001-qobuz-sync`
**Created**: 2026-02-14
**Status**: Draft
**Input**: User description: "I'd like a Rust CLI to automatically sync my purchases from Qobuz to a specified local directory. Don't rely on any obscure crates; it's fine to call the Qobuz HTTP endpoints directly. MP3 quality is fine."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - First-Time Full Sync (Priority: P1)

A user who has purchased music on Qobuz wants to download their entire purchased library to a local directory in one command. They provide their Qobuz credentials and a target directory, and the tool downloads every purchased album and track as MP3 files, organized into a predictable folder structure.

**Why this priority**: This is the core value proposition. Without this, the tool has no purpose. Many Qobuz users are frustrated with the official downloader's poor handling of large batch downloads, timeouts, and failures. A reliable one-command sync solves the primary pain point.

**Independent Test**: Can be fully tested by running the CLI against a Qobuz account with purchases and verifying all purchased tracks appear as MP3 files in the target directory with correct folder structure.

**Acceptance Scenarios**:

1. **Given** a Qobuz account with purchased albums and an empty target directory, **When** the user runs the sync command, **Then** all purchased tracks are downloaded as MP3 files organized by artist and album.
2. **Given** valid credentials and a target directory, **When** the sync completes, **Then** every purchased track exists as a playable MP3 file in the target directory.
3. **Given** a Qobuz account with both full album purchases and individual track purchases, **When** the user runs sync, **Then** both album tracks and standalone track purchases are downloaded.

---

### User Story 2 - Incremental Sync (Priority: P2)

A user who has previously synced wants to run the tool again after buying new music. The tool detects which tracks are already present locally and only downloads new purchases, avoiding redundant downloads and wasted bandwidth.

**Why this priority**: After the first sync, every subsequent use depends on incremental behavior. Without this, users must re-download their entire library each time, which is impractical for large collections.

**Independent Test**: Can be tested by running sync twice with a new purchase added between runs, verifying only the new purchase is downloaded on the second run.

**Acceptance Scenarios**:

1. **Given** a previously synced directory and no new purchases, **When** the user runs sync, **Then** no files are downloaded and the tool reports that the library is up to date.
2. **Given** a previously synced directory and one new album purchase, **When** the user runs sync, **Then** only the new album's tracks are downloaded.
3. **Given** a previously synced directory where a local file has been deleted, **When** the user runs sync, **Then** the deleted file is re-downloaded to restore completeness.

---

### User Story 3 - Progress and Error Visibility (Priority: P3)

A user syncing a large library wants to see progress and understand any issues. The tool displays what is being downloaded, how far along the sync is, and clearly reports any failures (network errors, unavailable tracks) without aborting the entire sync.

**Why this priority**: Large Qobuz libraries can contain hundreds of albums. Without progress visibility, users cannot distinguish a working sync from a hung process. Graceful error handling prevents a single failure from blocking the rest of the library.

**Independent Test**: Can be tested by running sync against a library with many albums and observing that progress output appears, and by simulating a network interruption to verify the tool continues with remaining tracks.

**Acceptance Scenarios**:

1. **Given** a library with multiple albums, **When** the user runs sync, **Then** the tool displays which album/track is currently being downloaded and overall progress.
2. **Given** a network interruption during download of one track, **When** the error occurs, **Then** the tool reports the failure and continues downloading remaining tracks.
3. **Given** a completed sync with some failures, **When** the sync finishes, **Then** the tool displays a summary listing successful downloads, skipped files, and failures.

---

### Edge Cases

- What happens when the user provides invalid or expired credentials? The tool must report a clear authentication error and exit before attempting any downloads.
- What happens when the target directory does not exist? The tool must create it (including intermediate directories).
- What happens when disk space runs out mid-download? The tool must report the error, clean up any partial file, and exit gracefully.
- What happens when a download is interrupted (partial file on disk)? Partial files must not be left behind; they should be cleaned up or resumed on next sync.
- What happens when Qobuz rate-limits or throttles requests? The tool must back off and retry with reasonable delays.
- What happens when the same track appears in multiple purchases (e.g., a single and an album)? The track should be stored once under its album, avoiding duplicates.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST authenticate the user with their Qobuz account credentials.
- **FR-002**: System MUST retrieve the complete list of the user's purchased albums and tracks from Qobuz.
- **FR-003**: System MUST download each purchased track in MP3 format (320 kbps preferred, highest available MP3 bitrate).
- **FR-004**: System MUST organize downloaded files into an `Album Artist / Album / Track` directory hierarchy within the target directory. For compilation or various-artists albums, the album artist directory (e.g., "Various Artists") is used, and each track filename includes the track artist: `NN - Track Artist - Title.mp3`.
- **FR-005**: System MUST name track files using the pattern `NN - Title.mp3` where NN is the zero-padded track number. For multi-disc albums, tracks MUST be placed in a `Disc N/` subdirectory within the album folder; single-disc albums remain flat.
- **FR-006**: System MUST skip tracks that already exist locally with the correct file size, enabling incremental sync.
- **FR-007**: System MUST re-download tracks whose local file is missing or has an incorrect size (partial/corrupt).
- **FR-008**: System MUST accept the target directory as a command-line argument.
- **FR-009**: System MUST accept Qobuz credentials via a config file (`~/.config/qoget/config.toml`) and/or environment variables (`QOBUZ_USERNAME`, `QOBUZ_PASSWORD`). Environment variables MUST take precedence over config file values. Credentials MUST NOT appear in shell history or CLI arguments.
- **FR-010**: System MUST display progress during sync: current album/track name, download count, and overall status.
- **FR-011**: System MUST continue syncing remaining tracks when an individual track download fails, and report all failures at completion.
- **FR-012**: System MUST clean up partial files when a download is interrupted or fails.
- **FR-013**: System MUST sanitize file and directory names to remove characters invalid on the target filesystem.
- **FR-014**: System MUST exit with a nonzero status code when any track fails to download.
- **FR-015**: System MUST download tracks using bounded parallelism with a small fixed concurrency limit (e.g., 4 simultaneous downloads) to balance throughput against server load.
- **FR-016**: System MUST support a `--dry-run` flag that lists all tracks that would be downloaded (with their target paths) without actually downloading any files.

### Key Entities

- **Purchase**: An album or individual track bought by the user on Qobuz. Has an artist, title, and one or more tracks.
- **Track**: An individual audio file within a purchase. Has a title, track number, duration, and is available for download in MP3 format.
- **Sync State**: The set of already-downloaded files in the target directory, determined by presence and file size, used to avoid redundant downloads.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can sync their entire Qobuz purchase library to a local directory with a single command invocation.
- **SC-002**: Subsequent sync runs complete in under 30 seconds when no new purchases exist (excluding network latency for purchase list retrieval).
- **SC-003**: All downloaded MP3 files are playable in standard audio players without corruption.
- **SC-004**: The tool correctly identifies and skips 100% of previously downloaded tracks during incremental sync.
- **SC-005**: When individual track downloads fail, the tool still downloads all other available tracks and reports failures clearly.
- **SC-006**: File organization follows a consistent, predictable directory structure that users can browse with any file manager.

## Assumptions

- The user has an active Qobuz account with at least one purchase.
- The user has network access to Qobuz services.
- Qobuz provides HTTP endpoints for authentication, listing purchases, and downloading tracks. The tool will interact with these endpoints directly.
- MP3 320 kbps is the target quality; the user does not require lossless formats.
- The tool is a command-line application implemented in Rust, per the user's explicit constraint.
- The tool will use well-established Rust crates (e.g., for HTTP, JSON, CLI argument parsing) rather than obscure or unmaintained libraries.
- Qobuz files are DRM-free, so downloaded MP3s require no additional processing to be playable.

## Clarifications

### Session 2026-02-14

- Q: Should tracks be downloaded sequentially or in parallel? → A: Bounded parallel (small fixed limit, e.g., 4 concurrent downloads).
- Q: How should multi-disc albums be organized? → A: Add `Disc N/` subdirectory only for multi-disc albums; single-disc albums stay flat.
- Q: How should compilation/various-artists albums be organized? → A: Use album artist for directory; include track artist in filename (`NN - Track Artist - Title.mp3`).
- Q: How should credentials be provided? → A: Config file (`~/.config/qoget/config.toml`) with env var override.
- Q: Should the tool support a dry-run/preview mode? → A: Yes, add `--dry-run` flag that lists planned downloads without downloading.
