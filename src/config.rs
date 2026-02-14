use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

pub struct Config {
    pub username: String,
    pub password: String,
    pub app_id: Option<String>,
    pub app_secret: Option<String>,
}

#[derive(Deserialize, Default)]
struct FileConfig {
    username: Option<String>,
    password: Option<String>,
    app_id: Option<String>,
    app_secret: Option<String>,
}

fn config_path() -> PathBuf {
    // XDG_CONFIG_HOME or ~/.config
    let config_dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_default();
            PathBuf::from(home).join(".config")
        });
    config_dir.join("qoget").join("config.toml")
}

pub fn load_config() -> Result<Config> {
    // Load file config if it exists
    let file_cfg = match std::fs::read_to_string(config_path()) {
        Ok(contents) => toml::from_str::<FileConfig>(&contents)
            .context("Failed to parse config file")?,
        Err(_) => FileConfig::default(),
    };

    // Env vars take precedence over file config
    let username = std::env::var("QOBUZ_USERNAME")
        .ok()
        .or(file_cfg.username);
    let password = std::env::var("QOBUZ_PASSWORD")
        .ok()
        .or(file_cfg.password);

    let username = match username {
        Some(u) if !u.is_empty() => u,
        _ => prompt_username()?,
    };
    let password = match password {
        Some(p) if !p.is_empty() => p,
        _ => prompt_password()?,
    };

    // app_id/app_secret: only from config file (no env var override needed)
    Ok(Config {
        username,
        password,
        app_id: file_cfg.app_id,
        app_secret: file_cfg.app_secret,
    })
}

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
