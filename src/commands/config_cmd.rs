//! `hypo config get/set` — 管理全局配置。

use crate::error::HypoError;

/// 获取配置项。
pub async fn get(key: &str) -> Result<(), HypoError> {
    let config_path = crate::paths::config_path();
    let content = std::fs::read_to_string(&config_path).unwrap_or_default();
    let config: crate::config::Config = toml::from_str(&content).unwrap_or_default();

    match key {
        "log_level" => println!("log_level = {}", config.log_level),
        "keyring_path" => println!("keyring_path = {}", config.keyring_path),
        "cache_dir" => println!("cache_dir = {}", config.cache_dir),
        _ => return Err(HypoError::Config(format!("未知配置项: {key}"))),
    }

    Ok(())
}

/// 设置配置项。
pub async fn set(key: &str, value: &str) -> Result<(), HypoError> {
    let config_path = crate::paths::config_path();
    let content = std::fs::read_to_string(&config_path).unwrap_or_default();
    let mut config: crate::config::Config = toml::from_str(&content).unwrap_or_default();

    match key {
        "log_level" => config.log_level = value.to_string(),
        "keyring_path" => config.keyring_path = value.to_string(),
        "cache_dir" => config.cache_dir = value.to_string(),
        _ => return Err(HypoError::Config(format!("未知配置项: {key}"))),
    }

    let toml_str = toml::to_string_pretty(&config)
        .map_err(|e| HypoError::Config(format!("序列化配置失败: {e}")))?;

    std::fs::write(&config_path, toml_str)
        .map_err(|e| HypoError::Config(format!("写入配置失败: {e}")))?;

    println!("{key} = {value}");
    Ok(())
}
