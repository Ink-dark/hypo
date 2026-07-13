//! `~/.hypo/` 目录结构管理。
//!
//! 跨平台定位 `~/.hypo/` 基础目录，
//! 提供各子目录与文件路径的获取函数，以及目录初始化。

use std::path::PathBuf;

use crate::constants::{
    CACHE_DIR_NAME, DEFAULT_CONFIG_FILENAME, DEFAULT_DB_FILENAME, KEYRING_DIR_NAME, LOGS_DIR_NAME,
    TMP_DIR_NAME,
};

/// 返回用户主目录。
fn home_dir() -> PathBuf {
    directories::UserDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .or_else(|| {
            std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .ok()
                .map(PathBuf::from)
        })
        .unwrap_or_else(|| PathBuf::from("."))
}

/// 返回 hypo 基础目录 `~/.hypo/`。
pub fn hypo_base_dir() -> PathBuf {
    home_dir().join(".hypo")
}

/// 配置文件路径：`~/.hypo/config.toml`。
pub fn config_path() -> PathBuf {
    hypo_base_dir().join(DEFAULT_CONFIG_FILENAME)
}

/// 缓存目录：`~/.hypo/cache/`。
pub fn cache_dir() -> PathBuf {
    hypo_base_dir().join(CACHE_DIR_NAME)
}

/// Keyring 目录：`~/.hypo/keyring/`。
pub fn keyring_dir() -> PathBuf {
    hypo_base_dir().join(KEYRING_DIR_NAME)
}

/// 临时目录：`~/.hypo/tmp/`。
pub fn tmp_dir() -> PathBuf {
    hypo_base_dir().join(TMP_DIR_NAME)
}

/// 数据库文件路径：`~/.hypo/hypo.db`。
pub fn db_path() -> PathBuf {
    hypo_base_dir().join(DEFAULT_DB_FILENAME)
}

/// 日志目录：`~/.hypo/logs/`。
pub fn logs_dir() -> PathBuf {
    hypo_base_dir().join(LOGS_DIR_NAME)
}

/// 创建全部必要的 hypo 目录（幂等，已存在则跳过）。
///
/// 在 `hypo init` 中调用。
pub fn ensure_dirs() -> std::io::Result<()> {
    let dirs = [
        hypo_base_dir(),
        cache_dir(),
        keyring_dir(),
        tmp_dir(),
        logs_dir(),
    ];
    for dir in &dirs {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_dir_exists() {
        let base = hypo_base_dir();
        assert!(base.ends_with(".hypo"));
    }

    #[test]
    fn test_all_paths_are_under_base() {
        let base = hypo_base_dir();
        assert!(config_path().starts_with(&base));
        assert!(cache_dir().starts_with(&base));
        assert!(keyring_dir().starts_with(&base));
        assert!(tmp_dir().starts_with(&base));
        assert!(db_path().starts_with(&base));
        assert!(logs_dir().starts_with(&base));
    }
}
