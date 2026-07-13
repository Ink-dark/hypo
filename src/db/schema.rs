//! 数据库表结构定义与初始化。
//!
//! 使用 SQLite（rusqlite bundled），存储已安装包与 registry 配置。

use std::path::Path;

use rusqlite::Connection;

use crate::error::HypoError;

/// 当前数据库 schema 版本号。
const SCHEMA_VERSION: u32 = 1;

/// 初始化数据库：创建表结构（IF NOT EXISTS）。
///
/// 若数据库文件不存在则自动创建。
pub fn init_db(path: &Path) -> Result<Connection, HypoError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| HypoError::Database(format!("创建数据库目录失败: {e}")))?;
    }

    let conn =
        Connection::open(path).map_err(|e| HypoError::Database(format!("打开数据库失败: {e}")))?;

    // 启用 WAL 模式以提高并发性能
    conn.execute_batch("PRAGMA journal_mode=WAL;")
        .map_err(|e| HypoError::Database(format!("设置 WAL 模式失败: {e}")))?;

    // 创建 packages 表
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS packages (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            owner       TEXT NOT NULL,
            name        TEXT NOT NULL,
            version     TEXT NOT NULL,
            platform    TEXT NOT NULL DEFAULT '',
            arch        TEXT NOT NULL DEFAULT '',
            install_path TEXT NOT NULL DEFAULT '',
            script_dir  TEXT NOT NULL DEFAULT '',
            source_registry TEXT NOT NULL DEFAULT '',
            installed_at TEXT NOT NULL DEFAULT (datetime('now')),
            latest_seen_version TEXT NOT NULL DEFAULT '',
            UNIQUE(owner, name)
        );",
    )
    .map_err(|e| HypoError::Database(format!("创建 packages 表失败: {e}")))?;

    // 创建 registries 表
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS registries (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL UNIQUE,
            base_pkg_url TEXT NOT NULL,
            is_official INTEGER NOT NULL DEFAULT 0,
            added_at    TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )
    .map_err(|e| HypoError::Database(format!("创建 registries 表失败: {e}")))?;

    // 创建 schema 版本表
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER NOT NULL
        );",
    )
    .map_err(|e| HypoError::Database(format!("创建 schema_version 表失败: {e}")))?;

    // 写入初始版本号
    let count: u32 = conn
        .query_row("SELECT COUNT(*) FROM schema_version", [], |row| row.get(0))
        .unwrap_or(0);
    if count == 0 {
        conn.execute(
            "INSERT INTO schema_version (version) VALUES (?1)",
            [SCHEMA_VERSION],
        )
        .map_err(|e| HypoError::Database(format!("写入 schema 版本失败: {e}")))?;
    }

    Ok(conn)
}

/// 数据库迁移——从旧版本升级到当前版本。
///
/// 通过 `schema_version` 表追踪版本号，按版本依次执行迁移。
pub fn migrate(conn: &Connection) -> Result<(), HypoError> {
    let version: u32 = conn
        .query_row("SELECT MAX(version) FROM schema_version", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    if version >= SCHEMA_VERSION {
        return Ok(());
    }

    // 版本 0 → 1：初始创建（已由 init_db 处理）
    tracing::info!("数据库 schema 从 v{version} 迁移到 v{SCHEMA_VERSION}");

    // 未来迁移在此处追加：
    // if version < 2 { conn.execute_batch("ALTER TABLE ...")?; }

    conn.execute("UPDATE schema_version SET version = ?1", [SCHEMA_VERSION])
        .map_err(|e| HypoError::Database(format!("更新 schema 版本失败: {e}")))?;

    Ok(())
}
