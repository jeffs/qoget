use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tokio::io::AsyncWriteExt;

use crate::bandcamp::{self, BandcampClient, BandcampPurchases};
use crate::client::QobuzClient;
use crate::models::{
    Album, AlbumId, Artist, BandcampCollectionItem, BandcampDownloadError, BandcampSyncResult,
    DiscNumber, DownloadError, DownloadTask, SyncPlan, SyncResult, Track, TrackId, TrackNumber,
};
use crate::path::{sanitize_component, track_path};

const CONCURRENT_DOWNLOADS: usize = 4;
const FORMAT_ID_MP3_320: u8 = 5;
const FORMAT_ID_CD_QUALITY: u8 = 6;

/// Result of a single track download indicating which format was used.
pub enum DownloadOutcome {
    Mp3,
    FlacFallback,
}

/// Execute all downloads in the sync plan with bounded parallelism and progress bars.
pub async fn execute_downloads(client: &QobuzClient, plan: SyncPlan) -> Result<SyncResult> {
    let skipped = plan.skipped;
    let total = plan.downloads.len() as u64;

    let multi = Arc::new(MultiProgress::new());
    let overall = multi.add(ProgressBar::new(total));
    overall.set_style(
        ProgressStyle::default_bar()
            .template("[{pos}/{len}] {msg}")
            .expect("valid template"),
    );

    let results: Vec<Result<(DownloadTask, DownloadOutcome), DownloadError>> =
        stream::iter(plan.downloads.into_iter().map(|task| {
            let multi = Arc::clone(&multi);
            let overall = overall.clone();
            async move {
                overall.set_message(format!("{} - {}", task.album.artist.name, task.track.title));

                let result = download_one(client, &task, &multi).await;
                overall.inc(1);

                match result {
                    Ok(outcome) => Ok((task, outcome)),
                    Err(e) => {
                        // Clean up temp files on failure (both .mp3.tmp and .flac.tmp)
                        for ext in [task.file_extension, ".flac"] {
                            let ext_no_dot = &ext[1..];
                            let temp_path =
                                task.target_path.with_extension(format!("{ext_no_dot}.tmp"));
                            let _ = tokio::fs::remove_file(&temp_path).await;
                        }
                        Err(DownloadError {
                            task,
                            error: format!("{e:#}"),
                        })
                    }
                }
            }
        }))
        .buffer_unordered(CONCURRENT_DOWNLOADS)
        .collect()
        .await;

    overall.finish_and_clear();

    let mut succeeded = Vec::new();
    let mut failed = Vec::new();
    let mut fallback_count = 0;
    for result in results {
        match result {
            Ok((task, outcome)) => {
                if matches!(outcome, DownloadOutcome::FlacFallback) {
                    fallback_count += 1;
                }
                succeeded.push(task);
            }
            Err(err) => failed.push(err),
        }
    }

    Ok(SyncResult {
        succeeded,
        failed,
        skipped,
        fallback_count,
    })
}

/// Download a single track: get URL (with format fallback), stream to temp file, rename to target.
///
/// Tries MP3 320 first. If the format request fails, retries with CD Quality (FLAC).
/// Returns which format was actually downloaded.
async fn download_one(
    client: &QobuzClient,
    task: &DownloadTask,
    multi: &MultiProgress,
) -> Result<DownloadOutcome> {
    // Try MP3 320, fall back to CD Quality on error
    let (url, outcome) = match client
        .get_file_url(task.track.id, FORMAT_ID_MP3_320)
        .await
    {
        Ok(url) => (url, DownloadOutcome::Mp3),
        Err(_mp3_err) => {
            eprintln!(
                "  MP3 unavailable, downloading CD Quality: {} - {}",
                task.album.artist.name, task.track.title
            );
            let url = client
                .get_file_url(task.track.id, FORMAT_ID_CD_QUALITY)
                .await
                .map_err(|cd_err| {
                    anyhow::anyhow!(
                        "unavailable in both MP3 320 and CD Quality: {cd_err:#}"
                    )
                })?;
            (url, DownloadOutcome::FlacFallback)
        }
    };

    // Determine actual target path (may differ from planned if fallback occurred)
    let actual_target = match outcome {
        DownloadOutcome::Mp3 => task.target_path.clone(),
        DownloadOutcome::FlacFallback => task.target_path.with_extension("flac"),
    };

    // Ensure parent directory exists
    if let Some(parent) = actual_target.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Download to temp file in same directory, then rename
    let actual_ext = match outcome {
        DownloadOutcome::Mp3 => task.file_extension,
        DownloadOutcome::FlacFallback => ".flac",
    };
    let ext_no_dot = &actual_ext[1..];
    let temp_path = actual_target.with_extension(format!("{ext_no_dot}.tmp"));

    let resp = client.http().get(&url).send().await?;

    if !resp.status().is_success() {
        anyhow::bail!("Download returned HTTP {}", resp.status());
    }

    // Set up per-file progress bar if content-length is known
    let content_len = resp.content_length();
    let pb = multi.add(ProgressBar::new(content_len.unwrap_or(0)));
    pb.set_style(
        ProgressStyle::default_bar()
            .template("  {bytes}/{total_bytes} {bar:30} {msg}")
            .expect("valid template"),
    );
    pb.set_message(task.track.title.clone());

    let bytes = resp.bytes().await?;
    pb.set_position(bytes.len() as u64);

    let mut file = tokio::fs::File::create(&temp_path).await?;
    file.write_all(&bytes).await?;
    file.flush().await?;
    drop(file);

    pb.finish_and_clear();

    // Atomic rename
    tokio::fs::rename(&temp_path, &actual_target).await?;

    Ok(outcome)
}

// --- Bandcamp download dispatch ---

/// Execute Bandcamp downloads: fetch download pages, download ZIPs, extract and place tracks.
///
/// Operates at the album/item level (not individual tracks) since Bandcamp delivers albums
/// as ZIP archives. For incremental sync, albums with existing .m4a files are skipped.
pub async fn execute_bandcamp_downloads(
    client: &BandcampClient,
    purchases: &BandcampPurchases,
    target_dir: &Path,
    dry_run: bool,
) -> Result<BandcampSyncResult> {
    let multi = Arc::new(MultiProgress::new());
    let overall = multi.add(ProgressBar::new(purchases.items.len() as u64));
    overall.set_style(
        ProgressStyle::default_bar()
            .template("[{pos}/{len}] {msg}")
            .expect("valid template"),
    );

    let mut result = BandcampSyncResult {
        downloaded: 0,
        skipped: 0,
        would_download: 0,
        failed: Vec::new(),
    };

    let temp_dir = target_dir.join(".qoget-temp");

    for item in &purchases.items {
        let desc = format!("{} - {}", item.band_name, item.item_title);
        overall.set_message(desc.clone());

        // Look up redownload URL by "{sale_item_type}{sale_item_id}" key
        let key = format!("{}{}", item.sale_item_type, item.sale_item_id);
        let redownload_url = match purchases.redownload_urls.get(&key) {
            Some(url) => url,
            None => {
                result.failed.push(BandcampDownloadError {
                    description: desc,
                    error: format!("No redownload URL found (key: {key})"),
                });
                overall.inc(1);
                continue;
            }
        };

        // Build album struct for path computation
        let album = Album {
            id: AlbumId(format!("bc-{}", item.item_id)),
            title: item.item_title.clone(),
            version: None,
            artist: Artist {
                id: item.sale_item_id,
                name: item.band_name.clone(),
            },
            media_count: 1,
            tracks_count: 0,
            tracks: None,
        };

        // Check if already synced
        if is_already_synced(target_dir, item, &album).await {
            result.skipped += 1;
            overall.inc(1);
            continue;
        }

        if dry_run {
            println!("{}", desc);
            result.would_download += 1;
            overall.inc(1);
            continue;
        }

        // Download
        tokio::fs::create_dir_all(&temp_dir).await?;
        match download_bandcamp_item(client, redownload_url, item, &album, target_dir, &temp_dir)
            .await
        {
            Ok(count) => result.downloaded += count,
            Err(e) => {
                result.failed.push(BandcampDownloadError {
                    description: desc,
                    error: format!("{e:#}"),
                });
            }
        }

        // Clean up temp files from this item
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;

        overall.inc(1);
    }

    overall.finish_and_clear();

    Ok(result)
}

/// Check if a Bandcamp item is already synced locally.
///
/// Checks the album directory for any .m4a files. Works for
/// both multi-track albums and single tracks since both end
/// up under `Artist/Title/`.
async fn is_already_synced(
    target_dir: &Path,
    _item: &BandcampCollectionItem,
    album: &Album,
) -> bool {
    let album_dir = target_dir
        .join(sanitize_component(&album.artist.name))
        .join(sanitize_component(&album.title));
    has_m4a_files(&album_dir).await
}

/// Download and extract a single Bandcamp item (album ZIP or single track).
async fn download_bandcamp_item(
    client: &BandcampClient,
    redownload_url: &str,
    item: &BandcampCollectionItem,
    album: &Album,
    target_dir: &Path,
    temp_dir: &Path,
) -> Result<usize> {
    // Fetch download page and get aac-hi URL
    let info = client.get_download_info(redownload_url).await?;
    let url = bandcamp::aac_hi_url(&info)?;

    // Download and extract
    let extracted = client.download_and_extract(url, temp_dir).await?;
    let mut count = 0;

    if extracted.len() > 1 {
        // Multi-track: use extracted track metadata for paths
        for ext_track in extracted {
            let track = Track {
                id: TrackId(
                    item.item_id
                        .wrapping_mul(1000)
                        .wrapping_add(ext_track.track_number as u64),
                ),
                title: ext_track.title,
                track_number: TrackNumber(ext_track.track_number),
                media_number: DiscNumber(1),
                duration: 0,
                performer: album.artist.clone(),
                isrc: None,
            };
            let target = track_path(target_dir, album, &track, ".m4a");
            if let Some(parent) = target.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::rename(&ext_track.temp_path, &target).await?;
            count += 1;
        }
    } else {
        // Single track: use item metadata for consistent path
        let track = Track {
            id: TrackId(item.item_id),
            title: item.item_title.clone(),
            track_number: TrackNumber(1),
            media_number: DiscNumber(1),
            duration: 0,
            performer: album.artist.clone(),
            isrc: None,
        };
        let target = track_path(target_dir, album, &track, ".m4a");
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        if let Some(ext_track) = extracted.into_iter().next() {
            tokio::fs::rename(&ext_track.temp_path, &target).await?;
            count += 1;
        }
    }

    Ok(count)
}

/// Check if a directory contains any .m4a files (non-recursive).
async fn has_m4a_files(dir: &Path) -> bool {
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return false;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        if entry.path().extension().and_then(|e| e.to_str()) == Some("m4a") {
            return true;
        }
    }
    false
}
