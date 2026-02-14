# Feature Specification: Bandcamp Support & Unified CLI

**Feature Branch**: `002-bandcamp-unified-cli`
**Created**: 2026-02-14
**Status**: Draft
**Input**: User description: "This tool automatically sync files from Qobuz. Add support for BandCamp, all behind a unified CLI."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Sync Bandcamp Purchases (Priority: P1)

A user who has purchased music on Bandcamp wants to download their entire Bandcamp purchase library to a local directory, just as they already can with Qobuz. They provide their Bandcamp credentials and a target directory, and the tool downloads every purchased album and track as AAC files (`.m4a`), organized into the same predictable folder structure used for Qobuz downloads.

**Why this priority**: This is the core new capability. Without Bandcamp download support, there is no feature to unify. A user who buys music on Bandcamp has no way to bulk-sync their library today using this tool.

**Independent Test**: Can be fully tested by running the sync command against a Bandcamp account with purchases and verifying all purchased tracks appear as `.m4a` files in the target directory with correct folder structure, independent of any Qobuz configuration.

**Acceptance Scenarios**:

1. **Given** a configured Bandcamp account with purchased albums, **When** the user runs sync targeting Bandcamp, **Then** all purchased tracks are downloaded as AAC files (`.m4a`) organized by artist and album.
2. **Given** a Bandcamp account with both full album purchases and individual track purchases, **When** the user runs sync, **Then** both album tracks and standalone track purchases are downloaded.
3. **Given** a previously synced Bandcamp library and no new purchases, **When** the user runs sync again, **Then** no files are downloaded and the tool reports the library is up to date.
4. **Given** a previously synced Bandcamp library and one new album purchase, **When** the user runs sync, **Then** only the new album's tracks are downloaded.

---

### User Story 2 - Unified Multi-Service Sync (Priority: P2)

A user who buys music on both Qobuz and Bandcamp wants to sync their entire collection from both services in a single command. They configure credentials for each service and run one sync command, and the tool downloads new purchases from all configured services into the same target directory with a consistent folder structure.

**Why this priority**: The "unified CLI" is the stated goal. Once Bandcamp support exists (P1), combining both services under one invocation is the natural next step. This eliminates the need to run separate tools or commands per service.

**Independent Test**: Can be tested by configuring both Qobuz and Bandcamp credentials, purchasing music on each, and running a single sync command, then verifying tracks from both services appear correctly in the target directory.

**Acceptance Scenarios**:

1. **Given** both Qobuz and Bandcamp credentials are configured and the user has purchases on both, **When** the user runs sync without specifying a service, **Then** purchases from both services are downloaded.
2. **Given** both services are configured but only one has new purchases, **When** the user runs sync, **Then** only the new tracks from the service with new purchases are downloaded.
3. **Given** an artist appears in purchases on both services with different albums, **When** the user runs sync, **Then** all albums from both services appear under the same artist directory.

---

### User Story 3 - Selective Service Sync (Priority: P3)

A user wants to sync only one of their configured services on a particular run. They specify which service to sync, and the tool downloads only from that service, ignoring others.

**Why this priority**: Useful for targeted syncing (e.g., after a Bandcamp Friday binge), but not essential for the core experience. All-service sync (P2) covers the default case.

**Independent Test**: Can be tested by configuring both services, running sync with a service filter, and verifying only the specified service's purchases are downloaded.

**Acceptance Scenarios**:

1. **Given** both services are configured, **When** the user runs sync specifying only Bandcamp, **Then** only Bandcamp purchases are downloaded.
2. **Given** both services are configured, **When** the user runs sync specifying only Qobuz, **Then** only Qobuz purchases are downloaded (existing behavior preserved).
3. **Given** the user specifies a service that is not configured, **When** the user runs sync, **Then** the tool reports a clear error indicating the service has no credentials configured.

---

### Edge Cases

- What happens when Bandcamp credentials are invalid or expired? The tool must report a clear authentication error for that service and continue syncing other configured services.
- What happens when one service fails entirely but others succeed? The tool must sync all working services and report which service(s) failed, with a nonzero exit code.
- What happens when Bandcamp returns a track with no downloadable AAC format? The tool must report the track as unavailable and continue with remaining tracks.
- What happens when no services are configured? The tool must report a clear error listing the supported services and how to configure them.
- What happens when the same artist name is spelled differently across services (e.g., "The National" on Qobuz vs "the national" on Bandcamp)? File organization uses the artist name as provided by each service; no cross-service normalization is attempted.
- What happens when a Bandcamp purchase is a "name your price" free download that the user claimed? It should be treated as a purchase and synced.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST support syncing purchases from Bandcamp, downloading tracks as AAC files (`.m4a`) using the same directory organization as Qobuz sync (`Artist / Album / NN - Title.m4a`).
- **FR-002**: System MUST support Bandcamp authentication via a session cookie string (`identity` cookie value) configured in the config file (`~/.config/qoget/config.toml`) and/or an environment variable. The environment variable MUST take precedence over the config file value. The user obtains this cookie from their browser's developer tools; it is a long-lived value that rarely needs updating.
- **FR-003**: System MUST retrieve the complete list of the user's purchased albums and tracks from Bandcamp.
- **FR-004**: System MUST apply the same incremental sync logic to Bandcamp downloads: skip tracks that already exist locally as non-empty files.
- **FR-005**: System MUST apply the same file/directory naming rules (sanitization, multi-disc handling, compilation artist attribution) to Bandcamp downloads as it does to Qobuz downloads.
- **FR-006**: When no service filter is specified, system MUST sync all configured services in a single invocation.
- **FR-007**: System MUST accept an optional service filter that limits sync to a specific service (e.g., only Bandcamp or only Qobuz).
- **FR-008**: System MUST continue syncing other services when one service fails, and report per-service results in the final summary.
- **FR-009**: System MUST display per-service progress during sync, so the user can see which service's purchases are currently being processed.
- **FR-010**: The `--dry-run` flag MUST work for Bandcamp and for multi-service sync, listing planned downloads from all targeted services.
- **FR-011**: Cross-service track deduplication is handled implicitly by file extension: Qobuz downloads produce `.mp3` files and Bandcamp downloads produce `.m4a` files. When both services have the same track, both versions coexist in the same album directory with different extensions. Within a single service, the existing file-path dedup (skip if non-empty file exists) applies as before.
- **FR-012**: System MUST preserve full backward compatibility with existing Qobuz-only configuration and behavior. A user with only Qobuz configured must see no change in behavior.

### Key Entities

- **Service**: A music purchase platform (Qobuz, Bandcamp) from which the user's library can be synced. Each service has its own authentication method and purchase retrieval mechanism.
- **Purchase**: An album or individual track bought by the user on any supported service. Normalized to a common representation regardless of originating service.
- **Track**: An individual audio file within a purchase. Regardless of service origin, has an artist, album, title, and track number. Download format depends on the service (MP3 for Qobuz, AAC for Bandcamp).
- **Sync Plan**: The set of tracks to download across all targeted services, after deduplication against local files. Cross-service duplicates coexist with different file extensions.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can sync their Bandcamp purchase library with a single command, just as they can with Qobuz today.
- **SC-002**: A user with both services configured can sync both libraries in a single command invocation.
- **SC-003**: Incremental sync for Bandcamp completes in under 30 seconds when no new purchases exist (excluding network latency for purchase list retrieval).
- **SC-004**: All Bandcamp-downloaded AAC files (`.m4a`) are playable in standard audio players without corruption.
- **SC-005**: Existing Qobuz-only users experience no change in behavior, configuration, or output after the update.
- **SC-006**: When one service fails during a multi-service sync, the tool still downloads all available tracks from other services and reports per-service outcomes.
- **SC-007**: File organization for Bandcamp downloads follows the same consistent, predictable directory structure as Qobuz downloads.

## Assumptions

- The user has an active Bandcamp account with at least one purchase that includes downloadable audio files.
- Bandcamp purchases include downloadable AAC files. Bandcamp offers multiple format choices; this tool targets AAC for Bandcamp downloads (`.m4a` extension), while Qobuz continues to use MP3.
- Bandcamp authentication uses the `identity` cookie value from the user's browser. The user copies this value from browser developer tools into the config file or environment variable. This is a long-lived cookie (valid for months); re-auth is only needed if the user logs out or changes their Bandcamp password.
- The tool name "qoget" may become a misnomer with multi-service support, but renaming is outside the scope of this feature. The tool identity and branding are a separate concern.
- Cross-service artist name normalization is not attempted. If an artist's name differs between services, each variant is treated as a separate artist directory. Users can reconcile manually or via a future feature.
- Bandcamp "name your price" / free downloads that the user has claimed are treated identically to paid purchases.

## Clarifications

### Session 2026-02-14

- Q: What form should the Bandcamp credential take? → A: Cookie string — user copies their `identity` cookie from browser devtools into config file or env var.
- Q: Should Bandcamp downloads use MP3 or a different format? → A: AAC (`.m4a`). Bandcamp downloads use AAC; Qobuz continues to use MP3. Cross-service duplicates coexist with different file extensions.
