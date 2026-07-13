//! Lockfile 生成、解析与完整性校验。
//!
//! `hypo.lock` 文件（TOML 格式）锁定依赖树的确切版本、下载 URL 与 SHA256。

use serde::{Deserialize, Serialize};

use crate::deps::resolver::ResolvedDep;
use crate::error::HypoError;

/// hypo.lock 文件内容。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lockfile {
    /// Schema 版本号。
    pub version: u32,
    /// 锁定条目列表。
    pub packages: Vec<LockfileEntry>,
}

/// 单个锁定条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockfileEntry {
    /// 包所有者。
    pub owner: String,
    /// 包名。
    pub name: String,
    /// 锁定的确切版本。
    pub version: String,
    /// .hypo 文件下载 URL。
    pub url: String,
    /// .hypo 文件 SHA256。
    pub sha256: String,
    /// 来源 registry URL。
    pub source_registry: String,
}

impl Lockfile {
    /// 从解析后的依赖树生成 lockfile 内容（TOML 字符串）。
    pub fn generate(resolved_deps: &[ResolvedDep]) -> Result<String, HypoError> {
        let packages: Vec<LockfileEntry> = resolved_deps
            .iter()
            .map(|d| LockfileEntry {
                owner: d.owner.clone(),
                name: d.name.clone(),
                version: d.version.clone(),
                url: String::new(),    // 下载 URL 需在安装时填入
                sha256: String::new(), // SHA256 需在下载后填入
                source_registry: String::new(),
            })
            .collect();

        let lock = Lockfile {
            version: 1,
            packages,
        };

        toml::to_string_pretty(&lock)
            .map_err(|e| HypoError::Config(format!("生成 lockfile 失败: {e}")))
    }

    /// 从 TOML 字符串解析 lockfile。
    pub fn parse(toml_str: &str) -> Result<Self, HypoError> {
        toml::from_str(toml_str).map_err(|e| HypoError::Config(format!("lockfile 解析失败: {e}")))
    }

    /// 在线模式完整性校验——对比 lockfile 与当前 registry 返回的值。
    ///
    /// 若 URL 或 SHA256 不一致，返回错误并提示用户重新解析依赖。
    pub fn verify_online(&self, _registry_url: &str) -> Result<(), HypoError> {
        // MVP 骨架：实际实现在 Step 8 完成，需要与 registry/client.rs 集成
        // 遍历 self.packages，回查 registry 验证 url/sha256 一致性
        Err(HypoError::Config(
            "lockfile 在线校验将在 Step 8 实现".to_string(),
        ))
    }

    /// 查找特定包的锁定条目。
    pub fn find(&self, owner: &str, name: &str) -> Option<&LockfileEntry> {
        self.packages
            .iter()
            .find(|e| e.owner == owner && e.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lockfile_roundtrip() {
        let deps = vec![
            ResolvedDep {
                owner: "alice".into(),
                name: "my-tool".into(),
                version: "1.2.3".into(),
                available_versions: vec!["1.2.3".into(), "1.0.0".into()],
            },
            ResolvedDep {
                owner: "bob".into(),
                name: "utils".into(),
                version: "2.0.0".into(),
                available_versions: vec!["2.0.0".into()],
            },
        ];

        let toml_str = Lockfile::generate(&deps).expect("生成失败");
        let restored = Lockfile::parse(&toml_str).expect("解析失败");

        assert_eq!(restored.version, 1);
        assert_eq!(restored.packages.len(), 2);
        assert_eq!(restored.packages[0].owner, "alice");
        assert_eq!(restored.packages[1].version, "2.0.0");
    }

    #[test]
    fn test_lockfile_find() {
        let deps = vec![ResolvedDep {
            owner: "alice".into(),
            name: "tool".into(),
            version: "1.0.0".into(),
            available_versions: vec![],
        }];
        let toml_str = Lockfile::generate(&deps).unwrap();
        let lock = Lockfile::parse(&toml_str).unwrap();

        assert!(lock.find("alice", "tool").is_some());
        assert!(lock.find("bob", "utils").is_none());
    }
}
