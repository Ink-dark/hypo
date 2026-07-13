use serde::{Deserialize, Serialize};

/// hypo 全局配置，对应 `~/.hypo/config.toml`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 受信任的用户列表（GitHub username）。
    #[serde(default)]
    pub trusted_users: Vec<String>,

    /// 自定义 registry 列表。
    #[serde(default)]
    pub custom_registries: Vec<String>,

    /// Keyring 目录路径。
    #[serde(default = "default_keyring_path")]
    pub keyring_path: String,

    /// 缓存目录路径。
    #[serde(default = "default_cache_dir")]
    pub cache_dir: String,

    /// 日志级别（trace / debug / info / warn / error）。
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_keyring_path() -> String {
    String::new()
}

fn default_cache_dir() -> String {
    String::new()
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            trusted_users: Vec::new(),
            custom_registries: Vec::new(),
            keyring_path: String::new(),
            cache_dir: String::new(),
            log_level: "info".to_string(),
        }
    }
}

impl Config {
    /// 从 `~/.hypo/config.toml` 加载配置。
    ///
    /// TODO: Step 8 配合 CLI init 实现。
    pub fn load() -> Self {
        todo!("Config::load() — 将在 Step 8 实现")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.log_level, "info");
        assert!(cfg.trusted_users.is_empty());
        assert!(cfg.custom_registries.is_empty());
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let cfg = Config {
            trusted_users: vec!["alice".into()],
            custom_registries: vec!["https://example.com/hypo".into()],
            keyring_path: "/home/user/.hypo/keyring".into(),
            cache_dir: "/home/user/.hypo/cache".into(),
            log_level: "debug".into(),
        };

        let toml_str = toml::to_string_pretty(&cfg).expect("序列化失败");
        let restored: Config = toml::from_str(&toml_str).expect("反序列化失败");

        assert_eq!(restored.trusted_users, cfg.trusted_users);
        assert_eq!(restored.custom_registries, cfg.custom_registries);
        assert_eq!(restored.log_level, "debug");
    }
}
