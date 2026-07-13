//! `hypo uninstall @owner/pkg` — 卸载包。

use crate::error::HypoError;
use crate::executor::executor_trait::ScriptExecutor;

/// 执行 hypo uninstall。
pub async fn run(package: &str) -> Result<(), HypoError> {
    let input = package.strip_prefix('@').unwrap_or(package);
    let (owner, name) = input
        .split_once('/')
        .ok_or_else(|| HypoError::Config(format!("无效的包名格式: {package}，应为 @owner/pkg")))?;

    let db_path = crate::paths::db_path();
    let conn = crate::db::schema::init_db(&db_path)?;

    // 查找已安装记录
    let record = crate::db::operations::get_package(&conn, owner, name)?
        .ok_or_else(|| HypoError::Config(format!("未找到已安装的包 @{owner}/{name}")))?;

    // 尝试执行卸载脚本（如果存在）
    let uninstall_script = std::path::Path::new(&record.script_dir).join("uninstall.ps1");
    if uninstall_script.exists() {
        let executor = crate::executor::powershell::PowerShellExecutor::new(
            &format!("@{owner}/{name}"),
            &record.version,
            record.install_path.clone().into(),
            std::path::PathBuf::new(),
        );
        // 卸载不需要确认
        // 执行卸载脚本
        let status = executor
            .execute(&uninstall_script, std::collections::HashMap::new())
            .await?;
        if !status.success() {
            eprintln!("警告: 卸载脚本以非零退出码结束");
        }
    }

    // 删除数据库记录
    crate::db::operations::delete_package(&conn, owner, name)?;

    println!("已卸载 @{owner}/{name}");

    Ok(())
}
