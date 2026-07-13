//! 已安装包与 registry 的 CRUD 操作。

use rusqlite::Connection;

use crate::error::HypoError;

/// 已安装包的数据库记录。
#[derive(Debug, Clone)]
pub struct PackageRecord {
    /// 主键。
    pub id: i64,
    /// 包所有者（如 `alice`）。
    pub owner: String,
    /// 包名（如 `my-tool`）。
    pub name: String,
    /// 已安装版本号。
    pub version: String,
    /// 平台（windows / linux / macos）。
    pub platform: String,
    /// 架构（x86_64 / aarch64）。
    pub arch: String,
    /// 安装目标路径。
    pub install_path: String,
    /// 脚本所在目录。
    pub script_dir: String,
    /// 来源 registry URL。
    pub source_registry: String,
    /// 安装时间（ISO 8601）。
    pub installed_at: String,
    /// 截至上次安装，该包在 registry 中的最新版本号（用于降级防护）。
    pub latest_seen_version: String,
}

/// Registry 记录。
#[derive(Debug, Clone)]
pub struct RegistryRecord {
    /// 主键。
    pub id: i64,
    /// Registry 名称。
    pub name: String,
    /// 基础 URL。
    pub base_pkg_url: String,
    /// 是否为官方目录。
    pub is_official: bool,
    /// 添加时间（ISO 8601）。
    pub added_at: String,
}

// ── packages 表 CRUD ──────────────────────────────────────────

/// 插入或替换已安装包记录。
///
/// 若同一 `(owner, name)` 已存在则更新所有字段。
pub fn insert_package(conn: &Connection, pkg: &PackageRecord) -> Result<(), HypoError> {
    conn.execute(
        "INSERT INTO packages (owner, name, version, platform, arch, install_path, script_dir, source_registry, latest_seen_version)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(owner, name) DO UPDATE SET
             version = excluded.version,
             platform = excluded.platform,
             arch = excluded.arch,
             install_path = excluded.install_path,
             script_dir = excluded.script_dir,
             source_registry = excluded.source_registry,
             latest_seen_version = excluded.latest_seen_version,
             installed_at = datetime('now')",
        rusqlite::params![
            pkg.owner,
            pkg.name,
            pkg.version,
            pkg.platform,
            pkg.arch,
            pkg.install_path,
            pkg.script_dir,
            pkg.source_registry,
            pkg.latest_seen_version,
        ],
    )
    .map_err(|e| HypoError::Database(format!("插入包记录失败: {e}")))?;

    Ok(())
}

/// 按 owner + name 查询包记录。
pub fn get_package(
    conn: &Connection,
    owner: &str,
    name: &str,
) -> Result<Option<PackageRecord>, HypoError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, owner, name, version, platform, arch, install_path, script_dir, source_registry, installed_at, latest_seen_version
             FROM packages WHERE owner = ?1 AND name = ?2",
        )
        .map_err(|e| HypoError::Database(format!("准备查询失败: {e}")))?;

    let result = stmt
        .query_row(rusqlite::params![owner, name], |row| {
            Ok(PackageRecord {
                id: row.get(0)?,
                owner: row.get(1)?,
                name: row.get(2)?,
                version: row.get(3)?,
                platform: row.get(4)?,
                arch: row.get(5)?,
                install_path: row.get(6)?,
                script_dir: row.get(7)?,
                source_registry: row.get(8)?,
                installed_at: row.get(9)?,
                latest_seen_version: row.get(10)?,
            })
        })
        .optional()
        .map_err(|e| HypoError::Database(format!("查询包记录失败: {e}")))?;

    Ok(result)
}

/// 列出全部已安装包。
pub fn list_packages(conn: &Connection) -> Result<Vec<PackageRecord>, HypoError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, owner, name, version, platform, arch, install_path, script_dir, source_registry, installed_at, latest_seen_version
             FROM packages ORDER BY owner, name",
        )
        .map_err(|e| HypoError::Database(format!("准备列表查询失败: {e}")))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(PackageRecord {
                id: row.get(0)?,
                owner: row.get(1)?,
                name: row.get(2)?,
                version: row.get(3)?,
                platform: row.get(4)?,
                arch: row.get(5)?,
                install_path: row.get(6)?,
                script_dir: row.get(7)?,
                source_registry: row.get(8)?,
                installed_at: row.get(9)?,
                latest_seen_version: row.get(10)?,
            })
        })
        .map_err(|e| HypoError::Database(format!("列出包记录失败: {e}")))?;

    let mut packages = Vec::new();
    for row in rows {
        packages.push(row.map_err(|e| HypoError::Database(format!("读取包记录行失败: {e}")))?);
    }

    Ok(packages)
}

/// 删除指定包记录。
pub fn delete_package(conn: &Connection, owner: &str, name: &str) -> Result<(), HypoError> {
    let affected = conn
        .execute(
            "DELETE FROM packages WHERE owner = ?1 AND name = ?2",
            rusqlite::params![owner, name],
        )
        .map_err(|e| HypoError::Database(format!("删除包记录失败: {e}")))?;

    if affected == 0 {
        return Err(HypoError::Database(format!(
            "未找到已安装的包 @{owner}/{name}"
        )));
    }

    Ok(())
}

/// 更新 latest_seen_version（安装完成后调用）。
pub fn update_latest_seen_version(
    conn: &Connection,
    owner: &str,
    name: &str,
    version: &str,
) -> Result<(), HypoError> {
    let affected = conn
        .execute(
            "UPDATE packages SET latest_seen_version = ?1 WHERE owner = ?2 AND name = ?3",
            rusqlite::params![version, owner, name],
        )
        .map_err(|e| HypoError::Database(format!("更新 latest_seen_version 失败: {e}")))?;

    if affected == 0 {
        return Err(HypoError::Database(format!(
            "未找到包 @{owner}/{name}，无法更新 latest_seen_version"
        )));
    }

    Ok(())
}

/// 获取 latest_seen_version（降级防护用）。
pub fn get_latest_seen_version(
    conn: &Connection,
    owner: &str,
    name: &str,
) -> Result<Option<String>, HypoError> {
    let mut stmt = conn
        .prepare("SELECT latest_seen_version FROM packages WHERE owner = ?1 AND name = ?2")
        .map_err(|e| HypoError::Database(format!("准备查询 latest_seen_version 失败: {e}")))?;

    let result = stmt
        .query_row(rusqlite::params![owner, name], |row| row.get(0))
        .optional()
        .map_err(|e| HypoError::Database(format!("查询 latest_seen_version 失败: {e}")))?;

    // 空字符串视为 None
    Ok(result.filter(|s: &String| !s.is_empty()))
}

// ── registries 表 CRUD ─────────────────────────────────────────

/// 插入或替换 registry 记录。
pub fn insert_registry(conn: &Connection, reg: &RegistryRecord) -> Result<(), HypoError> {
    conn.execute(
        "INSERT INTO registries (name, base_pkg_url, is_official, added_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(name) DO UPDATE SET
             base_pkg_url = excluded.base_pkg_url,
             is_official = excluded.is_official",
        rusqlite::params![
            reg.name,
            reg.base_pkg_url,
            reg.is_official as i32,
            reg.added_at
        ],
    )
    .map_err(|e| HypoError::Database(format!("插入 registry 记录失败: {e}")))?;

    Ok(())
}

/// 列出全部已配置的 registry。
pub fn list_registries(conn: &Connection) -> Result<Vec<RegistryRecord>, HypoError> {
    let mut stmt = conn
        .prepare(
            "SELECT id, name, base_pkg_url, is_official, added_at FROM registries ORDER BY name",
        )
        .map_err(|e| HypoError::Database(format!("准备 registry 列表查询失败: {e}")))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(RegistryRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                base_pkg_url: row.get(2)?,
                is_official: row.get::<_, i32>(3)? != 0,
                added_at: row.get(4)?,
            })
        })
        .map_err(|e| HypoError::Database(format!("列出 registry 记录失败: {e}")))?;

    let mut registries = Vec::new();
    for row in rows {
        registries
            .push(row.map_err(|e| HypoError::Database(format!("读取 registry 行失败: {e}")))?);
    }

    Ok(registries)
}

/// 删除 registry 记录。
pub fn delete_registry(conn: &Connection, name: &str) -> Result<(), HypoError> {
    let affected = conn
        .execute(
            "DELETE FROM registries WHERE name = ?1",
            rusqlite::params![name],
        )
        .map_err(|e| HypoError::Database(format!("删除 registry 记录失败: {e}")))?;

    if affected == 0 {
        return Err(HypoError::Database(format!("未找到 registry：{name}")));
    }

    Ok(())
}

// ── 辅助 trait ─────────────────────────────────────────────────

/// 避免直接引入 rusqlite::OptionalExtension。
trait OptionalExt<T> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for Result<T, rusqlite::Error> {
    fn optional(self) -> Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(v) => Ok(Some(v)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

// ── 单元测试 ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().expect("创建内存数据库失败");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS packages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                owner TEXT NOT NULL,
                name TEXT NOT NULL,
                version TEXT NOT NULL,
                platform TEXT NOT NULL DEFAULT '',
                arch TEXT NOT NULL DEFAULT '',
                install_path TEXT NOT NULL DEFAULT '',
                script_dir TEXT NOT NULL DEFAULT '',
                source_registry TEXT NOT NULL DEFAULT '',
                installed_at TEXT NOT NULL DEFAULT (datetime('now')),
                latest_seen_version TEXT NOT NULL DEFAULT '',
                UNIQUE(owner, name)
            );
            CREATE TABLE IF NOT EXISTS registries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                base_pkg_url TEXT NOT NULL,
                is_official INTEGER NOT NULL DEFAULT 0,
                added_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .expect("创建测试表失败");
        conn
    }

    fn make_pkg() -> PackageRecord {
        PackageRecord {
            id: 0,
            owner: "alice".into(),
            name: "test-pkg".into(),
            version: "1.0.0".into(),
            platform: "windows".into(),
            arch: "x86_64".into(),
            install_path: "/tmp/install".into(),
            script_dir: "/tmp/scripts".into(),
            source_registry: "https://alice.github.io/hypo-pkgs".into(),
            installed_at: String::new(),
            latest_seen_version: "1.0.0".into(),
        }
    }

    #[test]
    fn test_insert_and_get() {
        let conn = setup_db();
        let pkg = make_pkg();

        insert_package(&conn, &pkg).expect("插入失败");
        let got = get_package(&conn, "alice", "test-pkg")
            .expect("查询失败")
            .expect("未找到记录");

        assert_eq!(got.version, "1.0.0");
        assert_eq!(got.owner, "alice");
    }

    #[test]
    fn test_insert_upsert() {
        let conn = setup_db();
        let mut pkg = make_pkg();

        insert_package(&conn, &pkg).expect("插入失败");
        pkg.version = "2.0.0".into();
        insert_package(&conn, &pkg).expect("更新失败");

        let got = get_package(&conn, "alice", "test-pkg")
            .expect("查询失败")
            .expect("未找到记录");
        assert_eq!(got.version, "2.0.0");
    }

    #[test]
    fn test_get_nonexistent() {
        let conn = setup_db();
        let result = get_package(&conn, "nobody", "nothing").expect("查询失败");
        assert!(result.is_none());
    }

    #[test]
    fn test_list_and_delete() {
        let conn = setup_db();
        insert_package(&conn, &make_pkg()).expect("插入失败");

        let list = list_packages(&conn).expect("列表查询失败");
        assert_eq!(list.len(), 1);

        delete_package(&conn, "alice", "test-pkg").expect("删除失败");
        let list2 = list_packages(&conn).expect("列表查询失败");
        assert!(list2.is_empty());
    }

    #[test]
    fn test_latest_seen_version() {
        let conn = setup_db();
        insert_package(&conn, &make_pkg()).expect("插入失败");

        // 初始值
        let v = get_latest_seen_version(&conn, "alice", "test-pkg").expect("查询失败");
        assert_eq!(v, Some("1.0.0".into()));

        // 更新
        update_latest_seen_version(&conn, "alice", "test-pkg", "2.0.0").expect("更新失败");
        let v2 = get_latest_seen_version(&conn, "alice", "test-pkg").expect("查询失败");
        assert_eq!(v2, Some("2.0.0".into()));
    }
}
