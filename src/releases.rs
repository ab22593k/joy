use crate::config::ReleaseInfo;
use anyhow::{Context, Result};
use colored::Colorize;
use serde::Deserialize;
use std::path::PathBuf;

/// The Flutter release API returns a JSON object with releases key.
#[derive(Deserialize)]
struct FlutterReleasesResponse {
    releases: Vec<FlutterRelease>,
    base_url: Option<String>,
}

#[derive(Deserialize)]
struct FlutterRelease {
    version: String,
    channel: String,
    archive: String,
    sha256: String,
    release_date: String,
}

/// Path to the cached release list for the current platform.
pub(crate) fn releases_cache_path() -> PathBuf {
    let os = std::env::consts::OS;
    crate::config::releases_cache_dir().join(format!("releases_{os}.json"))
}

/// Save a release list to the disk cache.
fn save_cache(releases: &[ReleaseInfo]) {
    if let Ok(json) = serde_json::to_string(releases) {
        let dir = crate::config::releases_cache_dir();
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(releases_cache_path(), &json);
    }
}

/// Load a release list from the disk cache.
fn load_cache() -> Option<Vec<ReleaseInfo>> {
    let path = releases_cache_path();
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Fetch the list of Flutter releases from Google's storage API.
/// We pick the correct platform JSON (linux/macos/windows).
/// Falls back to the disk cache on network failure.
pub fn fetch_releases() -> Result<Vec<ReleaseInfo>> {
    let os = std::env::consts::OS;
    let url = match os {
        "linux" => {
            "https://storage.googleapis.com/flutter_infra_release/releases/releases_linux.json"
        }
        "macos" => {
            "https://storage.googleapis.com/flutter_infra_release/releases/releases_macos.json"
        }
        "windows" => {
            "https://storage.googleapis.com/flutter_infra_release/releases/releases_windows.json"
        }
        _ => anyhow::bail!("Unsupported OS: {os}"),
    };

    match fetch_releases_from_remote(url) {
        Ok(releases) => {
            save_cache(&releases);
            Ok(releases)
        }
        Err(remote_err) => {
            // Network failed — try the cache
            match load_cache() {
                Some(cached) => {
                    eprintln!(
                        "Warning: Could not fetch release list (offline?). Using cached data."
                    );
                    Ok(cached)
                }
                None => {
                    // No cache either — return the original error
                    Err(remote_err)
                }
            }
        }
    }
}

/// Fetch releases from the remote API, parsing the raw JSON response.
fn fetch_releases_from_remote(url: &str) -> Result<Vec<ReleaseInfo>> {
    let resp = reqwest::blocking::get(url).context("Failed to fetch Flutter releases list")?;
    let data: FlutterReleasesResponse = resp
        .json()
        .context("Failed to parse Flutter releases JSON")?;

    let releases: Vec<ReleaseInfo> = data
        .releases
        .into_iter()
        .map(|r| ReleaseInfo {
            version: r.version,
            channel: r.channel,
            archive_url: format!(
                "{}/{}",
                data.base_url
                    .as_deref()
                    .unwrap_or("https://storage.googleapis.com/flutter_infra_release/releases"),
                r.archive
            ),
            sha256: r.sha256,
            release_date: r.release_date,
        })
        .collect();

    Ok(releases)
}

/// Clear the cached release list.
pub fn clear_cache() -> Result<()> {
    let path = releases_cache_path();
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

/// Return the size of the cached release list in bytes.
pub fn cache_size() -> u64 {
    let path = releases_cache_path();
    if path.exists() {
        std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    }
}

/// Display the releases list to stdout
pub fn list_releases(show_all: bool) -> Result<()> {
    let releases = fetch_releases()?;
    let max_display = if show_all { releases.len() } else { 20 };

    println!("{}", "Available Flutter releases:".bold());
    for release in releases.iter().take(max_display) {
        let channel_color = match release.channel.as_str() {
            "stable" => "green",
            "beta" => "yellow",
            _ => "cyan",
        };
        println!(
            "  {} ({}) [{}] {}",
            release.version.bold(),
            release.channel.color(channel_color),
            release.release_date,
            release.archive_url.dimmed()
        );
    }

    if !show_all && releases.len() > max_display {
        println!(
            "  ... and {} more (use --all to see all)",
            releases.len() - max_display
        );
    }

    Ok(())
}

/// Find a release by version string (exact match or channel name).
pub fn find_release(version: &str) -> Result<ReleaseInfo> {
    let releases = fetch_releases()?;

    // Try exact match first
    if let Some(r) = releases.iter().find(|r| r.version == version) {
        return Ok(r.clone());
    }

    // Try channel match (latest in that channel)
    if let Some(r) = releases.iter().rev().find(|r| r.channel == version) {
        return Ok(r.clone());
    }

    anyhow::bail!(
        "Could not find Flutter version '{}'. Run 'dartup releases' to see available versions.",
        version
    )
}
