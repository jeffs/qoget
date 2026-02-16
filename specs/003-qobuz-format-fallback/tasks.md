# Tasks: Qobuz Format Fallback

**Input**: Design documents from `/specs/003-qobuz-format-fallback/`
**Prerequisites**: plan.md, spec.md, research.md

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2)
- Include exact file paths in descriptions

## Phase 1: Foundational

**Purpose**: Add types and constants needed by both user stories

- [x] T001 [P] Add `FORMAT_ID_CD_QUALITY: u8 = 6` constant and `DownloadOutcome` enum (`Mp3`, `FlacFallback`) in src/download.rs
- [x] T002 [P] Add `fallback_count: usize` field to `SyncResult` in src/models.rs

**Checkpoint**: New types compile, existing tests pass unchanged

---

## Phase 2: User Story 1 - Automatic Fallback to CD Quality (Priority: P1) MVP

**Goal**: Tracks unavailable in MP3 320 are automatically downloaded as CD Quality FLAC instead of failing

**Independent Test**: Run sync against account with format-restricted tracks; verify FLAC files appear for those tracks while MP3-available tracks remain `.mp3`

### Implementation for User Story 1

- [x] T003 [P] [US1] Modify `scan_existing()` in src/sync.rs to also check `.flac` extension for each task's target path — if either `.mp3` or `.flac` exists and is non-empty, mark as existing
- [x] T004 [US1] Implement fallback in `download_one()` in src/download.rs — call `get_file_url` with `FORMAT_ID_MP3_320`; on `Err`, retry with `FORMAT_ID_CD_QUALITY`; if fallback succeeds, rewrite target path from `.mp3` to `.flac`; return `DownloadOutcome` (depends on T001)
- [x] T005 [US1] Update `execute_downloads()` in src/download.rs to handle `DownloadOutcome` from `download_one()`, count fallback downloads, and populate `SyncResult.fallback_count` (depends on T001, T002, T004)

**Checkpoint**: Fallback downloads work end-to-end; re-running sync skips both `.mp3` and `.flac` tracks

---

## Phase 3: User Story 2 - Visibility of Format Decisions (Priority: P2)

**Goal**: Users can see which tracks used fallback and how many, from progress output and summary alone

**Independent Test**: Run sync with fallback tracks; verify per-track fallback message and summary count

### Implementation for User Story 2

- [x] T006 [US2] Add fallback progress message in `download_one()` in src/download.rs — when fallback occurs, emit "MP3 unavailable, downloading CD Quality: {artist} - {track}" via the progress bar or eprintln (depends on T004)
- [x] T007 [US2] Update Qobuz sync summary in src/main.rs to display fallback count in the summary line, e.g., "150 downloaded (3 as FLAC), 5 failed, 200 skipped" (depends on T002, T005)

**Checkpoint**: Progress output and summary both show fallback information

---

## Phase 4: Polish & Verification

**Purpose**: Confirm no regressions across existing functionality

- [x] T008 Verify all existing tests pass (`cargo test`), --dry-run is unaffected, and Bandcamp sync is unaffected

---

## Dependencies & Execution Order

### Phase Dependencies

- **Foundational (Phase 1)**: No dependencies — can start immediately
- **US1 (Phase 2)**: Depends on Phase 1 completion
- **US2 (Phase 3)**: Depends on T004 and T005 from Phase 2
- **Polish (Phase 4)**: Depends on all prior phases

### Parallel Opportunities

- T001 and T002 can run in parallel (different files)
- T003 can run in parallel with T004 (different files: sync.rs vs download.rs), but both need T001 complete first — T003 doesn't actually depend on T001 though, so T003 can run in parallel with Phase 1

### Within User Story 1

```text
T001 ──┐
       ├──→ T004 ──→ T005
T002 ──┘         ↗
T003 (parallel) ─┘
```

### Within User Story 2

```text
T004 ──→ T006
T005 ──→ T007
T002 ──→ T007
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Add types (T001, T002 in parallel)
2. Complete Phase 2: Fallback logic (T003 parallel with T004, then T005)
3. **STOP and VALIDATE**: Sync works with fallback; incremental sync recognizes both formats
4. Ready for real-world testing

### Incremental Delivery

1. Phase 1 + Phase 2 → Fallback works (MVP)
2. Phase 3 → Users see what happened (visibility)
3. Phase 4 → Confidence in no regressions

---

## Notes

- Total: 8 tasks across 4 phases
- US1: 3 implementation tasks (T003, T004, T005)
- US2: 2 implementation tasks (T006, T007)
- Foundational: 2 tasks (T001, T002)
- Polish: 1 task (T008)
- No new files created — all modifications to existing src/download.rs, src/sync.rs, src/models.rs, src/main.rs
- client.rs is already parameterized by format_id — no changes needed there
