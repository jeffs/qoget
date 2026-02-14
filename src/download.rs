use std::sync::Arc;

use anyhow::Result;
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tokio::io::AsyncWriteExt;

use crate::client::QobuzClient;
use crate::models::{DownloadError, DownloadTask, SyncPlan, SyncResult};

const CONCURRENT_DOWNLOADS: usize = 4;
const FORMAT_ID_MP3_320: u8 = 5;

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

    let results: Vec<Result<DownloadTask, DownloadError>> =
        stream::iter(plan.downloads.into_iter().map(|task| {
            let multi = Arc::clone(&multi);
            let overall = overall.clone();
            async move {
                overall.set_message(format!(
                    "{} - {}",
                    task.album.artist.name, task.track.title
                ));

                let result = download_one(client, &task, &multi).await;
                overall.inc(1);

                match result {
                    Ok(()) => Ok(task),
                    Err(e) => {
                        // Clean up temp file on failure
                        let temp_path = task.target_path.with_extension("mp3.tmp");
                        let _ = tokio::fs::remove_file(&temp_path).await;
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
    for result in results {
        match result {
            Ok(task) => succeeded.push(task),
            Err(err) => failed.push(err),
        }
    }

    Ok(SyncResult {
        succeeded,
        failed,
        skipped,
    })
}

/// Download a single track: get URL, stream to temp file, rename to target.
async fn download_one(
    client: &QobuzClient,
    task: &DownloadTask,
    multi: &MultiProgress,
) -> Result<()> {
    // Get download URL
    let url = client
        .get_file_url(task.track.id, FORMAT_ID_MP3_320)
        .await?;

    // Ensure parent directory exists
    if let Some(parent) = task.target_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Download to temp file in same directory, then rename
    let temp_path = task.target_path.with_extension("mp3.tmp");

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
    tokio::fs::rename(&temp_path, &task.target_path).await?;

    Ok(())
}
