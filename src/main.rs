use std::path::PathBuf;
use std::process;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use qoget::{bandcamp, bundle, client, config, download, models, sync};

#[derive(Parser)]
#[command(
    name = "qoget",
    about = "Sync purchased music from Qobuz and Bandcamp to a local directory"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Sync purchased music to a local directory
    ///
    /// Downloads from all configured services by default.
    /// Qobuz downloads MP3 320 (.mp3), Bandcamp downloads AAC (.m4a).
    ///
    /// Configure services in ~/.config/qoget/config.toml:
    ///
    ///   [qobuz]
    ///   username = "you@example.com"
    ///   password = "secret"
    ///
    ///   [bandcamp]
    ///   identity_cookie = "your-cookie"
    ///
    /// Or via environment variables: QOBUZ_USERNAME, QOBUZ_PASSWORD, BANDCAMP_IDENTITY
    Sync {
        /// Target directory for downloaded music
        target_dir: PathBuf,

        /// Preview what would be downloaded without downloading
        #[arg(long)]
        dry_run: bool,

        /// Sync only the specified service (qobuz or bandcamp)
        #[arg(long, value_name = "NAME")]
        service: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Sync {
            target_dir,
            dry_run,
            service,
        } => {
            if let Err(e) = run_sync(&target_dir, dry_run, service).await {
                eprintln!("Error: {e:#}");
                process::exit(1);
            }
        }
    }
}

fn parse_service(s: &str) -> Result<models::Service> {
    match s.to_lowercase().as_str() {
        "qobuz" => Ok(models::Service::Qobuz),
        "bandcamp" => Ok(models::Service::Bandcamp),
        _ => bail!("Unknown service '{s}'. Supported services: qobuz, bandcamp"),
    }
}

async fn run_sync(
    target_dir: &std::path::Path,
    dry_run: bool,
    service: Option<String>,
) -> Result<()> {
    let cfg = config::load_config()?;

    let service_filter = match service.as_deref() {
        Some(s) => Some(parse_service(s)?),
        None => None,
    };

    let should_run = |svc: models::Service| -> bool { service_filter.is_none_or(|f| f == svc) };

    let has_qobuz = cfg.qobuz.is_some();
    let has_bandcamp = cfg.bandcamp.is_some();

    if !has_qobuz && !has_bandcamp {
        if service_filter.is_some() && service_filter != Some(models::Service::Qobuz) {
            bail!(
                "Bandcamp is not configured.\n\n\
                 Add to ~/.config/qoget/config.toml:\n\n  \
                 [bandcamp]\n  \
                 identity_cookie = \"YOUR_COOKIE\"\n\n\
                 To get the cookie: log in to bandcamp.com, open browser dev tools (F12),\n\
                 go to Application > Cookies > bandcamp.com, and copy the 'identity' cookie value.\n\n\
                 Or set the BANDCAMP_IDENTITY environment variable."
            );
        }
        // Nothing configured from file/env — try interactive Qobuz login
        let qobuz_cfg = config::prompt_qobuz_credentials()?;
        eprintln!("Syncing Qobuz...");
        return run_qobuz_sync(qobuz_cfg, target_dir, dry_run).await;
    }

    let mut any_failure = false;

    if should_run(models::Service::Qobuz) {
        match cfg.qobuz {
            Some(qobuz_cfg) => {
                eprintln!("Syncing Qobuz...");
                if let Err(e) = run_qobuz_sync(qobuz_cfg, target_dir, dry_run).await {
                    eprintln!("Qobuz sync failed: {e:#}");
                    any_failure = true;
                }
            }
            None if service_filter.is_some() => {
                // User explicitly requested Qobuz — try prompting for credentials
                match config::prompt_qobuz_credentials() {
                    Ok(qobuz_cfg) => {
                        eprintln!("Syncing Qobuz...");
                        if let Err(e) = run_qobuz_sync(qobuz_cfg, target_dir, dry_run).await {
                            eprintln!("Qobuz sync failed: {e:#}");
                            any_failure = true;
                        }
                    }
                    Err(e) => bail!("Qobuz is not configured: {e:#}"),
                }
            }
            None => {}
        }
    }

    if should_run(models::Service::Bandcamp) {
        match cfg.bandcamp {
            Some(bandcamp_cfg) => {
                eprintln!("Syncing Bandcamp...");
                if let Err(e) = run_bandcamp_sync(bandcamp_cfg, target_dir, dry_run).await {
                    eprintln!("Bandcamp sync failed: {e:#}");
                    any_failure = true;
                }
            }
            None if service_filter.is_some() => {
                bail!(
                    "Bandcamp is not configured.\n\n\
                     Add to ~/.config/qoget/config.toml:\n\n  \
                     [bandcamp]\n  \
                     identity_cookie = \"YOUR_COOKIE\"\n\n\
                     To get the cookie: log in to bandcamp.com, open browser dev tools (F12),\n\
                     go to Application > Cookies > bandcamp.com, and copy the 'identity' cookie value.\n\n\
                     Or set the BANDCAMP_IDENTITY environment variable."
                );
            }
            None => {}
        }
    }

    // Hint about unconfigured services (only when no --service filter)
    if service_filter.is_none() {
        if !has_qobuz && has_bandcamp {
            eprintln!(
                "\nHint: Qobuz sync is also available. \
                 Set QOBUZ_USERNAME/QOBUZ_PASSWORD or add [qobuz] to config."
            );
        }
        if !has_bandcamp && has_qobuz {
            eprintln!(
                "\nHint: Bandcamp sync is also available. \
                 Set BANDCAMP_IDENTITY or add [bandcamp] to config."
            );
        }
    }

    if any_failure {
        bail!("One or more services failed");
    }

    Ok(())
}

async fn run_qobuz_sync(
    qobuz_cfg: config::QobuzConfig,
    target_dir: &std::path::Path,
    dry_run: bool,
) -> Result<()> {
    let http = reqwest::Client::new();

    let config::QobuzConfig {
        username,
        password,
        app_id,
        app_secret,
    } = qobuz_cfg;

    let creds = match (app_id, app_secret) {
        (Some(id), Some(secret)) => models::AppCredentials {
            app_id: id,
            app_secret: secret,
        },
        _ => {
            eprintln!("Extracting app credentials from Qobuz...");
            bundle::extract_credentials(&http).await?
        }
    };

    eprintln!("Logging in to Qobuz...");
    let auth = client::login(&http, &creds.app_id, &username, &password).await?;
    eprintln!("Logged in as user {}", auth.user_id);

    let qobuz = client::QobuzClient::new(http, creds.app_id, creds.app_secret, auth.token);

    eprintln!("Fetching Qobuz purchases...");
    let mut purchases = qobuz.get_purchases().await?;
    eprintln!(
        "Found {} albums and {} standalone tracks",
        purchases.albums.len(),
        purchases.tracks.len()
    );

    for album in &mut purchases.albums {
        if album.tracks.is_none() {
            let full = qobuz.get_album(&album.id).await?;
            album.tracks = full.tracks;
        }
    }

    let tasks = sync::collect_tasks(&purchases, target_dir, ".mp3");
    let existing = sync::scan_existing(&tasks).await;
    let plan = sync::build_sync_plan(tasks, &existing, dry_run);

    eprintln!(
        "{} tracks to download, {} already synced",
        plan.downloads.len(),
        plan.skipped.len()
    );

    if dry_run {
        for task in &plan.skipped {
            if matches!(task.reason, models::SkipReason::DryRun) {
                println!("{}", task.target_path.display());
            }
        }
        eprintln!(
            "\nDry run: {} tracks would be downloaded, {} already synced",
            plan.skipped
                .iter()
                .filter(|s| matches!(s.reason, models::SkipReason::DryRun))
                .count(),
            plan.skipped
                .iter()
                .filter(|s| matches!(s.reason, models::SkipReason::AlreadyExists))
                .count(),
        );
        return Ok(());
    }

    if plan.downloads.is_empty() {
        eprintln!("Qobuz library is up to date.");
        return Ok(());
    }

    let result = download::execute_downloads(&qobuz, plan).await?;

    if result.fallback_count > 0 {
        eprintln!(
            "\nQobuz: {} succeeded ({} as FLAC), {} failed, {} skipped",
            result.succeeded.len(),
            result.fallback_count,
            result.failed.len(),
            result.skipped.len()
        );
    } else {
        eprintln!(
            "\nQobuz: {} succeeded, {} failed, {} skipped",
            result.succeeded.len(),
            result.failed.len(),
            result.skipped.len()
        );
    }

    if !result.failed.is_empty() {
        eprintln!("\nFailed Qobuz downloads:");
        for err in &result.failed {
            eprintln!(
                "  {} - {}: {}",
                err.task.album.title, err.task.track.title, err.error
            );
        }
        bail!("Some Qobuz downloads failed");
    }

    Ok(())
}

async fn run_bandcamp_sync(
    bandcamp_cfg: config::BandcampConfig,
    target_dir: &std::path::Path,
    dry_run: bool,
) -> Result<()> {
    let bc_client = bandcamp::BandcampClient::new(bandcamp_cfg.identity_cookie)?;

    eprintln!("Verifying Bandcamp authentication...");
    let fan_id = bc_client.verify_auth().await?;
    eprintln!("Bandcamp fan_id: {fan_id}");

    eprintln!("Fetching Bandcamp purchases...");
    let purchases = bc_client.get_purchases(fan_id).await?;
    eprintln!(
        "Found {} Bandcamp items ({} with download URLs)",
        purchases.items.len(),
        purchases.redownload_urls.len()
    );

    let result =
        download::execute_bandcamp_downloads(&bc_client, &purchases, target_dir, dry_run).await?;

    if dry_run {
        eprintln!(
            "\nDry run: {} would be downloaded, {} already synced",
            result.would_download, result.skipped
        );
    } else {
        eprintln!(
            "\nBandcamp: {} tracks downloaded, {} already synced",
            result.downloaded, result.skipped
        );
    }

    if !result.failed.is_empty() {
        eprintln!("\nFailed Bandcamp downloads:");
        for err in &result.failed {
            eprintln!("  {}: {}", err.description, err.error);
        }
        bail!("Some Bandcamp downloads failed");
    }

    Ok(())
}
