use std::path::PathBuf;
use std::process;

use anyhow::Result;
use clap::{Parser, Subcommand};
use qoget::{bundle, client, config, download, models, sync};

#[derive(Parser)]
#[command(name = "qoget", about = "Sync Qobuz purchases to local directory")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Sync purchased music to a local directory
    Sync {
        /// Target directory for downloaded music
        target_dir: PathBuf,

        /// Preview what would be downloaded without downloading
        #[arg(long)]
        dry_run: bool,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Sync { target_dir, dry_run } => {
            if let Err(e) = run_sync(&target_dir, dry_run).await {
                eprintln!("Error: {e:#}");
                process::exit(1);
            }
        }
    }
}

async fn run_sync(target_dir: &std::path::Path, dry_run: bool) -> Result<()> {
    // 1. Load config
    let cfg = config::load_config()?;

    let http = reqwest::Client::new();

    // 2. Get app credentials (from config or bundle extraction)
    let creds = match (cfg.app_id, cfg.app_secret) {
        (Some(id), Some(secret)) => models::AppCredentials {
            app_id: id,
            app_secret: secret,
        },
        _ => {
            eprintln!("Extracting app credentials from Qobuz...");
            bundle::extract_credentials(&http).await?
        }
    };

    // 3. Login
    eprintln!("Logging in...");
    let auth = client::login(&http, &creds.app_id, &cfg.username, &cfg.password).await?;
    eprintln!("Logged in as user {}", auth.user_id);

    // 4. Build API client
    let qobuz = client::QobuzClient::new(
        http,
        creds.app_id,
        creds.app_secret,
        auth.token,
    );

    // 5. Fetch purchases
    eprintln!("Fetching purchases...");
    let mut purchases = qobuz.get_purchases().await?;
    eprintln!(
        "Found {} albums and {} standalone tracks",
        purchases.albums.len(),
        purchases.tracks.len()
    );

    // 6. Populate tracks for each album
    for album in &mut purchases.albums {
        if album.tracks.is_none() {
            let full = qobuz.get_album(&album.id).await?;
            album.tracks = full.tracks;
        }
    }

    // 7. Scan existing files and build sync plan
    let preliminary = sync::collect_tasks(&purchases, target_dir);
    let existing = sync::scan_existing(&preliminary).await;
    let plan = sync::build_sync_plan(&purchases, target_dir, &existing, dry_run);

    eprintln!(
        "{} tracks to download, {} already synced",
        plan.downloads.len(),
        plan.skipped.len()
    );

    // 8. Dry-run: list what would be downloaded and exit
    if dry_run {
        for task in &plan.skipped {
            if matches!(task.reason, models::SkipReason::DryRun) {
                println!("{}", task.target_path.display());
            }
        }
        eprintln!(
            "\nDry run: {} tracks would be downloaded, {} already synced",
            plan.skipped.iter().filter(|s| matches!(s.reason, models::SkipReason::DryRun)).count(),
            plan.skipped.iter().filter(|s| matches!(s.reason, models::SkipReason::AlreadyExists)).count(),
        );
        return Ok(());
    }

    if plan.downloads.is_empty() {
        eprintln!("Library is up to date.");
        return Ok(());
    }

    // 9. Execute downloads
    let result = download::execute_downloads(&qobuz, plan).await?;

    // 10. Print summary
    eprintln!(
        "\nDone: {} succeeded, {} failed, {} skipped",
        result.succeeded.len(),
        result.failed.len(),
        result.skipped.len()
    );

    if !result.failed.is_empty() {
        eprintln!("\nFailed downloads:");
        for err in &result.failed {
            eprintln!("  {} - {}: {}", err.task.album.title, err.task.track.title, err.error);
        }
        process::exit(1);
    }

    Ok(())
}
