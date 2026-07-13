//! `hypo list` — 列出已安装包。

use crate::error::HypoError;

/// 执行 hypo list。
pub async fn run() -> Result<(), HypoError> {
    let db_path = crate::paths::db_path();
    let conn = crate::db::schema::init_db(&db_path)?;
    let packages = crate::db::operations::list_packages(&conn)?;

    if packages.is_empty() {
        println!("暂无已安装的包。使用 `hypo install @owner/pkg` 安装。");
        return Ok(());
    }

    println!(
        "{:<30} {:<12} {:<12} {:<20}",
        "包名", "版本", "平台", "来源"
    );
    println!("{}", "-".repeat(74));
    for pkg in &packages {
        let full_name = format!("@{}/{}", pkg.owner, pkg.name);
        println!(
            "{:<30} {:<12} {:<12} {:<20}",
            full_name, pkg.version, pkg.platform, pkg.source_registry
        );
    }

    Ok(())
}
