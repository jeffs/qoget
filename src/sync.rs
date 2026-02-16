use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::models::{
    Album, AlbumId, DownloadTask, PurchaseList, SkipReason, SkippedTrack, SyncPlan, Track, TrackId,
};
use crate::path::track_path;

/// Set of local files that exist and are non-empty.
pub struct ExistingFiles(HashSet<PathBuf>);

/// Alternative extensions to check when determining if a track already exists.
/// Handles format fallback: a task planned as `.mp3` may already exist as `.flac`.
const ALT_EXTENSIONS: &[&str] = &[".flac", ".mp3"];

/// Scan the target paths in the plan and stat each one.
/// Also checks alternative extensions (e.g., `.flac` for a `.mp3` task) so that
/// tracks downloaded via format fallback are recognized as already synced.
/// This is the only I/O in the sync module — keeps build_sync_plan pure.
pub async fn scan_existing(tasks: &[DownloadTask]) -> ExistingFiles {
    let mut existing = HashSet::new();
    for task in tasks {
        if file_exists_nonempty(&task.target_path).await {
            existing.insert(task.target_path.clone());
            continue;
        }
        // Check alternative extensions (e.g., .flac when task targets .mp3)
        for alt_ext in ALT_EXTENSIONS {
            if *alt_ext == task.file_extension {
                continue;
            }
            let alt_path = task.target_path.with_extension(&alt_ext[1..]);
            if file_exists_nonempty(&alt_path).await {
                // Record the original planned path so build_sync_plan marks it as skipped
                existing.insert(task.target_path.clone());
                break;
            }
        }
    }
    ExistingFiles(existing)
}

async fn file_exists_nonempty(path: &Path) -> bool {
    tokio::fs::metadata(path)
        .await
        .is_ok_and(|m| m.is_file() && m.len() > 0)
}

/// Build a sync plan from pre-built download tasks. Pure function — no I/O.
///
/// Deduplicates by TrackId: if the same track appears in multiple purchases
/// (e.g., as a standalone single and within an album), keeps the album version
/// (prefers the DownloadTask whose album has more than one track).
///
/// After dedup, classifies each task as download or skip based on:
/// - existing files (non-empty) → SkipReason::AlreadyExists
/// - dry_run mode → SkipReason::DryRun
pub fn build_sync_plan(
    tasks: Vec<DownloadTask>,
    existing: &ExistingFiles,
    dry_run: bool,
) -> SyncPlan {
    // Deduplicate by TrackId: prefer album version (album with tracks_count > 1)
    let mut best: HashMap<TrackId, DownloadTask> = HashMap::new();
    for task in tasks {
        let id = task.track.id;
        match best.get(&id) {
            Some(existing_task)
                if existing_task.album.tracks_count > 1 && task.album.tracks_count <= 1 =>
            {
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

/// Build a list of download tasks from purchases.
/// Used to get target paths for scan_existing and as input to build_sync_plan.
pub fn collect_tasks(
    purchases: &PurchaseList,
    base_dir: &Path,
    ext: &'static str,
) -> Vec<DownloadTask> {
    let mut all_tasks: Vec<DownloadTask> = Vec::new();

    for album in &purchases.albums {
        if let Some(ref paginated) = album.tracks {
            for track in &paginated.items {
                let target = track_path(base_dir, album, track, ext);
                all_tasks.push(DownloadTask {
                    track: track.clone(),
                    album: album.clone(),
                    target_path: target,
                    file_extension: ext,
                });
            }
        }
    }

    // Standalone track purchases
    for track in &purchases.tracks {
        let album = standalone_album(track);
        let target = track_path(base_dir, &album, track, ext);
        all_tasks.push(DownloadTask {
            track: track.clone(),
            album,
            target_path: target,
            file_extension: ext,
        });
    }

    all_tasks
}

/// Create a minimal album struct for standalone track purchases.
fn standalone_album(track: &Track) -> Album {
    Album {
        id: AlbumId(format!("standalone-{}", track.id)),
        title: track.title.clone(),
        version: None,
        artist: track.performer.clone(),
        media_count: 1,
        tracks_count: 1,
        tracks: None,
    }
}
