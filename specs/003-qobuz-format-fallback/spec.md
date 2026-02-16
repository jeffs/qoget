# Feature Specification: Qobuz Format Fallback

**Feature Branch**: `003-qobuz-format-fallback`
**Created**: 2026-02-14
**Status**: Draft
**Input**: User description: "When MP3 is unavailable on Qobuz, we should fall back to CD Quality."

## User Scenarios & Testing

### User Story 1 - Automatic Fallback to CD Quality (Priority: P1)

A user syncs their Qobuz purchases. Some tracks are not available in MP3 320 format (Qobuz returns an error or empty URL for that format). Instead of failing those tracks, qoget automatically retries with CD Quality (lossless) and downloads the track successfully.

The user sees a brief note in the progress output indicating which tracks fell back to a different format, but no manual intervention is required.

**Why this priority**: This is the core value — tracks that currently fail due to format unavailability are downloaded instead of skipped.

**Independent Test**: Run `qoget sync <dir>` against an account that owns tracks unavailable in MP3 320. Verify those tracks are downloaded in CD Quality (FLAC, `.flac` extension) while MP3-available tracks remain as `.mp3`.

**Acceptance Scenarios**:

1. **Given** a purchased track available in MP3 320, **When** syncing, **Then** it downloads as `.mp3` (unchanged behavior)
2. **Given** a purchased track unavailable in MP3 320 but available in CD Quality, **When** syncing, **Then** it downloads as `.flac` with a note in progress output
3. **Given** a purchased track unavailable in both MP3 320 and CD Quality, **When** syncing, **Then** the track is reported as failed with a clear error listing which formats were attempted
4. **Given** a previously synced `.mp3` track, **When** syncing again, **Then** it is skipped as usual (no re-download in a different format)
5. **Given** a track that was previously downloaded as `.flac` via fallback, **When** syncing again, **Then** it is skipped (incremental sync recognizes it)

---

### User Story 2 - Visibility of Format Decisions (Priority: P2)

A user wants to understand which format each track was downloaded in, especially when fallback occurred. The progress output and summary show format information so the user can see what happened.

**Why this priority**: Transparency about what the tool did is important but secondary to actually getting the tracks downloaded.

**Independent Test**: Run a sync where some tracks fall back to CD Quality. Verify the summary mentions how many tracks used the fallback format.

**Acceptance Scenarios**:

1. **Given** a sync run where some tracks fell back to CD Quality, **When** the sync completes, **Then** the summary line includes a count of fallback downloads (e.g., "150 downloaded (3 as FLAC), 5 failed, 200 skipped")
2. **Given** a track that falls back during download, **When** the fallback occurs, **Then** a progress message is shown (e.g., "MP3 unavailable, downloading CD Quality: Artist - Track")

---

### Edge Cases

- When Qobuz returns a URL for MP3 but the URL itself 404s, the tool should treat this as a transient error and retry with existing retry logic, not immediately fall back to CD Quality. Format fallback only applies when Qobuz explicitly indicates the format is unavailable (error response from the format request, not from the download itself).
- When a directory already contains `Artist/Album/01 - Track.mp3` and the same track needs fallback to FLAC, the `.mp3` version takes precedence — the track is considered already synced. The tool does not download a second copy in a different format.
- `--dry-run` cannot know in advance which tracks will need fallback (it requires actually requesting the download URL). Dry-run should report tracks as planned downloads without attempting format resolution.
- Some tracks in the same album may end up as `.mp3` and others as `.flac`. This is acceptable — the goal is to get every track, not format consistency within an album.

## Requirements

### Functional Requirements

- **FR-001**: When a Qobuz track download URL request fails for MP3 320 format, the system MUST automatically retry with CD Quality format before reporting failure
- **FR-002**: The fallback order MUST be: MP3 320 first, then CD Quality. No other formats are attempted
- **FR-003**: Tracks downloaded via fallback MUST use the `.flac` file extension instead of `.mp3`
- **FR-004**: Incremental sync MUST recognize both `.mp3` and `.flac` files for the same track position as "already synced" — checking both extensions before deciding to download
- **FR-005**: The progress output MUST indicate when a track is being downloaded via format fallback
- **FR-006**: The sync summary MUST report a separate count of tracks downloaded via fallback
- **FR-007**: Existing behavior for tracks available in MP3 320 MUST be completely unchanged
- **FR-008**: Bandcamp sync MUST be completely unaffected by this change
- **FR-009**: `--dry-run` mode MUST continue to work without attempting format resolution (it lists planned downloads based on file existence checks only)

## Success Criteria

### Measurable Outcomes

- **SC-001**: 100% of tracks available in at least one supported format (MP3 320 or CD Quality) are successfully downloaded — zero format-related failures for purchasable content
- **SC-002**: Sync time for a library with no format-unavailable tracks is unchanged (no extra requests when MP3 succeeds on first try)
- **SC-003**: Users can identify which tracks were downloaded via fallback from the sync output alone
- **SC-004**: Re-running sync after a run with fallback downloads results in zero new downloads (incremental sync works for both formats)

## Assumptions

- Qobuz format ID 5 corresponds to MP3 320 and format ID 6 corresponds to CD Quality (FLAC 16-bit/44.1kHz)
- When a format is unavailable, Qobuz returns a distinguishable error response from the download URL endpoint (not a valid URL that later 404s)
- CD Quality (FLAC) is very broadly available on Qobuz — it is rare for a track to be unavailable in both MP3 and CD Quality
- The FLAC files will be larger than MP3 equivalents. Users accept this tradeoff implicitly (the fallback is automatic, not opt-in)

## Out of Scope

- User-configurable preferred format or format priority ordering
- Downloading in multiple formats simultaneously
- Converting between formats (e.g., transcoding FLAC to MP3)
- Hi-res (24-bit) format support
- Bandcamp format changes
