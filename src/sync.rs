use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::models::{
    Album, DownloadTask, PurchaseList, SkipReason, SkippedTrack, SyncPlan, Track, TrackId,
};
use crate::path::track_path;

/// Set of local files that exist and are non-empty.
pub struct ExistingFiles(HashSet<PathBuf>);

/// Scan the target paths in the plan and stat each one.
/// This is the only I/O in the sync module — keeps build_sync_plan pure.
pub async fn scan_existing(tasks: &[DownloadTask]) -> ExistingFiles {
    let mut existing = HashSet::new();
    for task in tasks {
        if let Ok(meta) = tokio::fs::metadata(&task.target_path).await
            && meta.is_file()
            && meta.len() > 0
        {
            existing.insert(task.target_path.clone());
        }
    }
    ExistingFiles(existing)
}

/// Build a sync plan from purchases. Pure function — no I/O.
///
/// Deduplicates by TrackId: if the same track appears in multiple purchases
/// (e.g., as a standalone single and within an album), keeps the album version
/// (prefers the DownloadTask whose album has more than one track).
///
/// After dedup, classifies each task as download or skip based on:
/// - existing files (non-empty) → SkipReason::AlreadyExists
/// - dry_run mode → SkipReason::DryRun
pub fn build_sync_plan(
    purchases: &PurchaseList,
    base_dir: &Path,
    existing: &ExistingFiles,
    dry_run: bool,
) -> SyncPlan {
    // Collect all (track, album) pairs
    let mut all_tasks: Vec<DownloadTask> = Vec::new();

    for album in &purchases.albums {
        if let Some(ref paginated) = album.tracks {
            for track in &paginated.items {
                let target = track_path(base_dir, album, track);
                all_tasks.push(DownloadTask {
                    track: track.clone(),
                    album: album.clone(),
                    target_path: target,
                });
            }
        }
    }

    // Standalone track purchases
    for track in &purchases.tracks {
        let target = track_path_standalone(base_dir, track);
        all_tasks.push(DownloadTask {
            track: track.clone(),
            album: standalone_album(track),
            target_path: target,
        });
    }

    // Deduplicate by TrackId: prefer album version (album with tracks_count > 1)
    let mut best: HashMap<TrackId, DownloadTask> = HashMap::new();
    for task in all_tasks {
        let id = task.track.id;
        match best.get(&id) {
            Some(existing) if existing.album.tracks_count > 1 && task.album.tracks_count <= 1 => {
                // Keep the existing album version over a standalone
            }
            _ => {
                best.insert(id, task);
            }
        }
    }

    let deduped: Vec<DownloadTask> = best.into_values().collect();
    let total_tracks = deduped.len();

    let mut downloads = Vec::new();
    let mut skipped = Vec::new();

    for task in deduped {
        if existing.0.contains(&task.target_path) {
            skipped.push(SkippedTrack {
                track: task.track,
                target_path: task.target_path,
                reason: SkipReason::AlreadyExists,
            });
        } else if dry_run {
            skipped.push(SkippedTrack {
                track: task.track,
                target_path: task.target_path,
                reason: SkipReason::DryRun,
            });
        } else {
            downloads.push(task);
        }
    }

    SyncPlan {
        downloads,
        skipped,
        total_tracks,
    }
}

/// Build a preliminary list of download tasks (before skip/dry-run classification).
/// Used to get target paths for scan_existing.
pub fn collect_tasks(purchases: &PurchaseList, base_dir: &Path) -> Vec<DownloadTask> {
    let mut all_tasks: Vec<DownloadTask> = Vec::new();

    for album in &purchases.albums {
        if let Some(ref paginated) = album.tracks {
            for track in &paginated.items {
                let target = track_path(base_dir, album, track);
                all_tasks.push(DownloadTask {
                    track: track.clone(),
                    album: album.clone(),
                    target_path: target,
                });
            }
        }
    }

    for track in &purchases.tracks {
        let target = track_path_standalone(base_dir, track);
        all_tasks.push(DownloadTask {
            track: track.clone(),
            album: standalone_album(track),
            target_path: target,
        });
    }

    all_tasks
}

/// Build path for a standalone track purchase (no album context from API).
fn track_path_standalone(base_dir: &Path, track: &Track) -> PathBuf {
    track_path(base_dir, &standalone_album(track), track)
}

/// Create a minimal album struct for standalone track purchases.
fn standalone_album(track: &Track) -> Album {
    Album {
        id: crate::models::AlbumId(format!("standalone-{}", track.id)),
        title: track.title.clone(),
        version: None,
        artist: track.performer.clone(),
        media_count: 1,
        tracks_count: 1,
        tracks: None,
    }
}
