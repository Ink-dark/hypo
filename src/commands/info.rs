//! `hypo info @owner/pkg` — 查看包详情。

use crate::error::HypoError;

/// 执行 hypo info。
pub async fn run(package: &str) -> Result<(), HypoError> {
    let (owner, name) = parse_package(package)?;

    println!("包信息: @{owner}/{name}");
    println!("{}", "-".repeat(40));

    // 尝试从本地数据库获取
    let db_path = crate::paths::db_path();
    let conn = crate::db::schema::init_db(&db_path)?;
    if let Some(record) = crate::db::operations::get_package(&conn, owner, name)? {
        println!("  已安装版本: {}", record.version);
        println!("  平台:       {}", record.platform);
        println!("  安装路径:   {}", record.install_path);
        println!("  安装时间:   {}", record.installed_at);
        println!("  最新版本:   {}", record.latest_seen_version);
        println!("  来源:       {}", record.source_registry);
    } else {
        println!("  (未安装)");
    }

    Ok(())
}

/// 解析 @owner/pkg 格式。
fn parse_package(input: &str) -> Result<(&str, &str), HypoError> {
    let input = input.strip_prefix('@').unwrap_or(input);
    input
        .split_once('/')
        .ok_or_else(|| HypoError::Config(format!("无效的包名格式: {input}，应为 @owner/pkg")))
}
