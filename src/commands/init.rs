//! `hypo init` — 初始化本地配置与目录结构。

use crate::error::HypoError;
use crate::paths;

/// 执行 hypo init。
pub async fn run() -> Result<(), HypoError> {
    // 创建目录结构
    paths::ensure_dirs().map_err(|e| HypoError::Config(format!("创建目录失败: {e}")))?;

    // 初始化 SQLite 数据库
    let db_path = paths::db_path();
    crate::db::schema::init_db(&db_path)?;

    // 写入默认配置（若不存在）
    let config_path = paths::config_path();
    if !config_path.exists() {
        let default_config = crate::config::Config::default();
        let toml_str = toml::to_string_pretty(&default_config)
            .map_err(|e| HypoError::Config(format!("序列化默认配置失败: {e}")))?;
        std::fs::write(&config_path, toml_str)
            .map_err(|e| HypoError::Config(format!("写入配置文件失败: {e}")))?;
    }

    println!("hypo 初始化完成");
    println!("  配置目录: {}", paths::hypo_base_dir().display());
    println!("  数据库:   {}", db_path.display());

    Ok(())
}
