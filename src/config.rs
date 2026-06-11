use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Return an XDG base-directories instance scoped to "dartup".
/// Panics if $HOME is not set.
fn xdg() -> xdg::BaseDirectories {
    xdg::BaseDirectories::with_prefix("dartup")
}

/// Root of user-specific data (installed SDKs, profiles, default symlink).
/// `$XDG_DATA_HOME/dartup` or `~/.local/share/dartup`.
pub fn data_root() -> PathBuf {
    xdg()
        .get_data_home()
        .expect("$HOME must be set to use dartup")
}

/// Root of user-specific cache (engine artifacts, git objects, temp downloads).
/// `$XDG_CACHE_HOME/dartup` or `~/.cache/dartup`.
pub fn cache_root() -> PathBuf {
    xdg()
        .get_cache_home()
        .expect("$HOME must be set to use dartup")
}

/// Directory where Flutter SDK versions are installed: `{data_root}/envs`
pub fn envs_dir() -> PathBuf {
    data_root().join("envs")
}

/// Directory for shared engine artifact cache: `{cache_root}/engines`
pub fn engine_cache_dir() -> PathBuf {
    cache_root().join("engines")
}

/// Directory for shared git data (bare repo cache): `{cache_root}/git`
pub fn git_cache_dir() -> PathBuf {
    cache_root().join("git")
}

/// Path to the global default symlink: `{data_root}/default`
pub fn global_default_path() -> PathBuf {
    data_root().join("default")
}

/// Temporary download directory: `{cache_root}/tmp`
pub fn tmp_dir() -> PathBuf {
    cache_root().join("tmp")
}

/// Per-project config file name
pub const PROJECT_CONFIG_FILE: &str = ".dartup.json";

/// Directory name for override storage
pub const OVERRIDE_DIR: &str = ".dartup";

/// Override file name inside .dartup/
pub const OVERRIDE_FILE: &str = "override";

/// Path to the override file for a given project directory
pub fn override_path(project_root: &std::path::Path) -> std::path::PathBuf {
    project_root.join(OVERRIDE_DIR).join(OVERRIDE_FILE)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReleaseInfo {
    pub version: String,
    pub channel: String,
    pub archive_url: String,
    pub sha256: String,
    pub release_date: String,
}

// ── Migration from legacy ~/.dartup/ ──

/// Legacy `~/.dartup` path — only checked for migration purposes.
fn legacy_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("~"))
        .join(".dartup")
}

/// Marker file written after successful migration.
/// Exists at `{data_root}/.xdg-migrated`.
fn migrated_marker() -> PathBuf {
    data_root().join(".xdg-migrated")
}

/// Migrate legacy `~/.dartup/` layout to XDG Base Directory paths.
/// Safe to call multiple times — checks for marker file.
pub fn migrate_if_needed() -> std::io::Result<()> {
    let legacy = legacy_home();
    if !legacy.exists() {
        return Ok(());
    }
    if migrated_marker().exists() {
        return Ok(());
    }

    let data = data_root();
    let cache = cache_root();

    fn migrate_dir(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
        if !src.exists() {
            return Ok(());
        }
        if dst.exists() {
            return Ok(());
        }
        std::fs::create_dir_all(dst.parent().unwrap())?;
        copy_dir(src, dst)?;
        std::fs::remove_dir_all(src)?;
        Ok(())
    }

    fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            if file_type.is_dir() {
                copy_dir(&src_path, &dst_path)?;
            } else {
                std::fs::copy(&src_path, &dst_path)?;
            }
        }
        Ok(())
    }

    migrate_dir(&legacy.join("envs"), &data.join("envs"))?;
    migrate_dir(
        &legacy.join("cache").join("engines"),
        &cache.join("engines"),
    )?;
    migrate_dir(&legacy.join("cache").join("git"), &cache.join("git"))?;

    let legacy_default = legacy.join("default");
    let data_default = data.join("default");
    if legacy_default.exists()
        && let Ok(target) = std::fs::read_link(&legacy_default)
    {
        if let Some(ver_name) = target.file_name() {
            let new_target = data.join("envs").join(ver_name);
            let _ = std::fs::remove_file(&data_default);
            #[cfg(unix)]
            std::os::unix::fs::symlink(&new_target, &data_default).ok();
            #[cfg(not(unix))]
            let _ = new_target;
        }
        std::fs::remove_file(&legacy_default).ok();
    }

    let dead_engine = legacy.join("cache").join("engine");
    if dead_engine.exists() {
        std::fs::remove_dir_all(&dead_engine).ok();
    }

    std::fs::remove_dir(legacy.join("cache")).ok();
    std::fs::remove_dir(&legacy).ok();

    std::fs::create_dir_all(&data)?;
    std::fs::write(migrated_marker(), "migrated")?;

    println!("✅ Migrated ~/.dartup/ to XDG Base Directory layout.");
    println!("   SDKs → {}", data.join("envs").display());
    println!("   Cache → {}", cache.display());

    Ok(())
}
