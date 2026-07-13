//! 硬编码常量：官方目录 URL、公钥指纹集合、默认文件名等。
//!
//! 公钥指纹为占位值，后续替换真实指纹。
//! 官方目录 URL 可通过 `HYPO_REGISTRY_URL` 环境变量覆盖（用于测试）。

/// 官方目录根 URL（可通过 `HYPO_REGISTRY_URL` 环境变量覆盖）。
pub fn official_dir_url() -> String {
    std::env::var("HYPO_REGISTRY_URL")
        .unwrap_or_else(|_| "https://hypo-org.github.io/directory".to_string())
}

/// 官方签名公钥指纹集合（占位值，后续替换真实指纹）。
pub const OFFICIAL_KEY_FINGERPRINTS: &[&str] = &["PLACEHOLDER_FINGERPRINT_1"];

/// 公钥轮换过渡期（天）。
pub const KEY_TRANSITION_PERIOD_DAYS: u32 = 90;

/// 默认配置文件名。
pub const DEFAULT_CONFIG_FILENAME: &str = "config.toml";

/// 默认数据库文件名。
pub const DEFAULT_DB_FILENAME: &str = "hypo.db";

/// 缓存目录名。
pub const CACHE_DIR_NAME: &str = "cache";

/// Keyring 目录名。
pub const KEYRING_DIR_NAME: &str = "keyring";

/// 临时目录名。
pub const TMP_DIR_NAME: &str = "tmp";

/// 日志目录名。
pub const LOGS_DIR_NAME: &str = "logs";
