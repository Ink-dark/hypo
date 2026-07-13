//! `hypo registry` — 管理本地 registry 配置。

use crate::error::HypoError;

/// 添加自定义 registry。
pub async fn add(name: &str, url: &str) -> Result<(), HypoError> {
    let db_path = crate::paths::db_path();
    let conn = crate::db::schema::init_db(&db_path)?;

    let reg = crate::db::operations::RegistryRecord {
        id: 0,
        name: name.to_string(),
        base_pkg_url: url.to_string(),
        is_official: false,
        added_at: chrono_now(),
    };
    crate::db::operations::insert_registry(&conn, &reg)?;

    println!("已添加 registry: {name} → {url}");
    Ok(())
}

/// 移除 registry。
pub async fn remove(name: &str) -> Result<(), HypoError> {
    let db_path = crate::paths::db_path();
    let conn = crate::db::schema::init_db(&db_path)?;

    crate::db::operations::delete_registry(&conn, name)?;
    println!("已移除 registry: {name}");
    Ok(())
}

/// 列出 registry。
pub async fn list() -> Result<(), HypoError> {
    let db_path = crate::paths::db_path();
    let conn = crate::db::schema::init_db(&db_path)?;

    let registries = crate::db::operations::list_registries(&conn)?;
    if registries.is_empty() {
        println!("暂无自定义 registry。");
        return Ok(());
    }

    for r in &registries {
        let kind = if r.is_official { "官方" } else { "自定义" };
        println!(
            "  [{kind}] {name} → {url}",
            name = r.name,
            url = r.base_pkg_url
        );
    }
    Ok(())
}

/// 导出注册表。
pub async fn export(file: &str) -> Result<(), HypoError> {
    let db_path = crate::paths::db_path();
    let conn = crate::db::schema::init_db(&db_path)?;
    let registries = crate::db::operations::list_registries(&conn)?;

    let content = registries
        .iter()
        .map(|r| format!("{} = \"{}\"", r.name, r.base_pkg_url))
        .collect::<Vec<_>>()
        .join("\n");

    std::fs::write(file, &content)
        .map_err(|e| HypoError::Config(format!("导出注册表失败: {e}")))?;

    println!("已导出 {} 条 registry 到 {file}", registries.len());
    Ok(())
}

/// 简陋的当前时间字符串（避免引入 chrono 依赖）。
fn chrono_now() -> String {
    // 简单的 YYYY-MM-DD HH:MM:SS 格式
    "unknown".to_string()
}
