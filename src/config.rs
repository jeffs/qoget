use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

// --- Public config types ---

pub struct Config {
    pub qobuz: Option<QobuzConfig>,
    pub bandcamp: Option<BandcampConfig>,
}

pub struct QobuzConfig {
    pub username: String,
    pub password: String,
    pub app_id: Option<String>,
    pub app_secret: Option<String>,
}

pub struct BandcampConfig {
    pub identity_cookie: String,
}

// --- TOML deserialization types ---

#[derive(Deserialize, Default)]
struct FileConfig {
    // New format: [qobuz] and [bandcamp] sections
    qobuz: Option<QobuzFileSection>,
    bandcamp: Option<BandcampFileSection>,
    // Old format: bare keys (backward compat for Qobuz)
    username: Option<String>,
    password: Option<String>,
    app_id: Option<String>,
    app_secret: Option<String>,
}

#[derive(Deserialize)]
struct QobuzFileSection {
    username: Option<String>,
    password: Option<String>,
    app_id: Option<String>,
    app_secret: Option<String>,
}

#[derive(Deserialize)]
struct BandcampFileSection {
    identity_cookie: Option<String>,
}

// --- File helpers ---

fn qobuz_username_from_file(fc: &FileConfig) -> Option<String> {
    fc.qobuz
        .as_ref()
        .and_then(|q| q.username.clone())
        .or_else(|| fc.username.clone())
        .filter(|s| !s.is_empty())
}

fn qobuz_password_from_file(fc: &FileConfig) -> Option<String> {
    fc.qobuz
        .as_ref()
        .and_then(|q| q.password.clone())
        .or_else(|| fc.password.clone())
        .filter(|s| !s.is_empty())
}

fn qobuz_app_id_from_file(fc: &FileConfig) -> Option<String> {
    fc.qobuz
        .as_ref()
        .and_then(|q| q.app_id.clone())
        .or_else(|| fc.app_id.clone())
}

fn qobuz_app_secret_from_file(fc: &FileConfig) -> Option<String> {
    fc.qobuz
        .as_ref()
        .and_then(|q| q.app_secret.clone())
        .or_else(|| fc.app_secret.clone())
}

fn bandcamp_identity_from_file(fc: &FileConfig) -> Option<String> {
    fc.bandcamp
        .as_ref()
        .and_then(|b| b.identity_cookie.clone())
        .filter(|s| !s.is_empty())
}

// --- Resolution (file only, no env vars) ---

fn resolve_qobuz_from_file(fc: &FileConfig) -> Option<QobuzConfig> {
    Some(QobuzConfig {
        username: qobuz_username_from_file(fc)?,
        password: qobuz_password_from_file(fc)?,
        app_id: qobuz_app_id_from_file(fc),
        app_secret: qobuz_app_secret_from_file(fc),
    })
}

fn resolve_bandcamp_from_file(fc: &FileConfig) -> Option<BandcampConfig> {
    Some(BandcampConfig {
        identity_cookie: bandcamp_identity_from_file(fc)?,
    })
}

// --- Resolution (with env vars) ---

fn resolve_qobuz(fc: &FileConfig) -> Option<QobuzConfig> {
    let username = std::env::var("QOBUZ_USERNAME")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| qobuz_username_from_file(fc))?;
    let password = std::env::var("QOBUZ_PASSWORD")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| qobuz_password_from_file(fc))?;
    Some(QobuzConfig {
        username,
        password,
        app_id: qobuz_app_id_from_file(fc),
        app_secret: qobuz_app_secret_from_file(fc),
    })
}

fn resolve_bandcamp(fc: &FileConfig) -> Option<BandcampConfig> {
    let identity_cookie = std::env::var("BANDCAMP_IDENTITY")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| bandcamp_identity_from_file(fc))?;
    Some(BandcampConfig { identity_cookie })
}

// --- Public API ---

fn config_path() -> PathBuf {
    let config_dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_default();
            PathBuf::from(home).join(".config")
        });
    config_dir.join("qoget").join("config.toml")
}

/// Parse config from TOML content only (no env vars, no prompts).
/// Exposed for testing.
pub fn parse_toml_config(content: &str) -> Result<Config> {
    let fc: FileConfig = toml::from_str(content).context("Failed to parse config")?;
    Ok(Config {
        qobuz: resolve_qobuz_from_file(&fc),
        bandcamp: resolve_bandcamp_from_file(&fc),
    })
}

/// Load config from file and env vars.
///
/// Precedence for each field:
/// 1. Environment variables (QOBUZ_USERNAME, QOBUZ_PASSWORD, BANDCAMP_IDENTITY)
/// 2. Config file [service] section
/// 3. Config file bare keys (Qobuz only, backward compat)
///
/// Returns whatever is fully resolved. Interactive prompts are NOT done here;
/// callers that need Qobuz can call `prompt_qobuz_credentials()` separately.
pub fn load_config() -> Result<Config> {
    let file_contents = match std::fs::read_to_string(config_path()) {
        Ok(c) => c,
        Err(_) => String::new(),
    };
    let fc: FileConfig =
        toml::from_str(&file_contents).context("Failed to parse config file")?;

    Ok(Config {
        qobuz: resolve_qobuz(&fc),
        bandcamp: resolve_bandcamp(&fc),
    })
}

/// Interactively prompt for missing Qobuz credentials, reusing any partial
/// values already resolved from env/file.
pub fn prompt_qobuz_credentials() -> Result<QobuzConfig> {
    let file_contents = match std::fs::read_to_string(config_path()) {
        Ok(c) => c,
        Err(_) => String::new(),
    };
    let fc: FileConfig =
        toml::from_str(&file_contents).context("Failed to parse config file")?;

    let username = std::env::var("QOBUZ_USERNAME")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| qobuz_username_from_file(&fc));
    let password = std::env::var("QOBUZ_PASSWORD")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| qobuz_password_from_file(&fc));

    let username = match username {
        Some(u) => u,
        None => prompt_username()?,
    };
    let password = match password {
        Some(p) => p,
        None => prompt_password()?,
    };

    Ok(QobuzConfig {
        username,
        password,
        app_id: qobuz_app_id_from_file(&fc),
        app_secret: qobuz_app_secret_from_file(&fc),
    })
}

// --- Interactive prompts ---

fn prompt_username() -> Result<String> {
    if !io::stdin().is_terminal() {
        bail!(
            "No username provided. Set QOBUZ_USERNAME or add username to \
             ~/.config/qoget/config.toml"
        );
    }
    eprint!("Qobuz email: ");
    io::stderr().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim().to_string();
    if trimmed.is_empty() {
        bail!("Username cannot be empty");
    }
    Ok(trimmed)
}

fn prompt_password() -> Result<String> {
    if !io::stdin().is_terminal() {
        bail!(
            "No password provided. Set QOBUZ_PASSWORD or add password to \
             ~/.config/qoget/config.toml"
        );
    }
    eprint!("Qobuz password: ");
    io::stderr().flush()?;
    let password = rpassword::read_password().context("Failed to read password")?;
    if password.is_empty() {
        bail!("Password cannot be empty");
    }
    Ok(password)
}
