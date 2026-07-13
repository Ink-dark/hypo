//! Registry 相关的 serde 数据结构。
//!
//! 对应 SPEC 7.1-7.4 定义的 JSON schema，
//! 全部结构体支持 `Serialize` / `Deserialize`。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ── 第一层：官方目录 ────────────────────────────────────────

/// 官方目录顶层索引 `registry.json`（SPEC 7.1）。
///
/// 包含分片列表 + 分片哈希表 + 快照版本 + 公钥轮换信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryJson {
    /// Schema 版本号。
    pub schema_version: u32,

    /// 快照版本，每次 PR 合并递增，用于增量同步。
    pub snapshot_version: u64,

    /// 所有分片（首字母）列表。
    pub shards: Vec<String>,

    /// 每个分片文件的 SHA256 哈希。
    pub shard_hashes: HashMap<String, String>,

    /// 官方签名公钥指纹集合。
    pub official_key_fingerprints: Vec<String>,

    /// 公钥轮换过渡期配置。
    pub key_rotation: KeyRotation,

    /// 公钥更新 URL。
    pub key_update_url: String,

    /// 镜像源列表。
    #[serde(default)]
    pub mirrors: Vec<String>,
}

/// 公钥轮换过渡期配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRotation {
    /// 旧密钥指纹。
    pub old_key_fingerprint: String,

    /// 旧密钥退役时间（ISO 8601）。
    pub old_key_retired_at: String,

    /// 过渡期天数（固定 90 天）。
    pub transition_period_days: u32,
}

// ── 第一层：开发者分片 ──────────────────────────────────────

/// 开发者分片信息（SPEC 7.2）。
///
/// 如 `a/alice.json` 中包含的开发者注册信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardJson {
    /// GitHub 用户名。
    pub github_username: String,

    /// GPG 公钥指纹列表。
    pub gpg_key_fingerprints: Vec<String>,

    /// 开发者 registry 基础 URL（gh-pages 根）。
    pub base_pkg_url: String,

    /// 注册时间（ISO 8601）。
    pub registered_at: String,
}

// ── 第二层：开发者 registry ─────────────────────────────────

/// 开发者所有包的顶层索引 `hypo-index.json`（SPEC 7.3）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypoIndex {
    /// Schema 版本号。
    pub schema_version: u32,

    /// 开发者 GitHub 用户名。
    pub owner: String,

    /// 包列表。
    pub packages: Vec<HypoIndexPackage>,
}

/// hypo-index.json 中的单个包条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypoIndexPackage {
    /// 包名。
    pub name: String,

    /// 包描述。
    #[serde(default)]
    pub description: String,

    /// GitHub 仓库 `owner/repo`。
    #[serde(default)]
    pub repo: String,

    /// 最新版本号。
    pub latest_version: String,

    /// 所有版本列表。
    pub versions: Vec<HypoIndexVersion>,
}

/// 包的一个版本条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypoIndexVersion {
    /// 版本号（SemVer）。
    pub version: String,

    /// 发布日期（ISO 8601）。
    pub released_at: String,

    /// 指向 hypo-package.json 的相对路径。
    pub package_index_path: String,

    /// 指向 manifest.toml 的相对路径。
    pub manifest_path: String,

    /// 指向 manifest.toml.sig 的相对路径。
    pub manifest_sig_path: String,

    /// 是否冻结。
    #[serde(default)]
    pub freeze: bool,

    /// 冻结原因（freeze=true 时必填）。
    #[serde(default)]
    pub freeze_reason: Option<String>,

    /// 回退版本号（freeze=true 时必填）。
    #[serde(default)]
    pub rollback_version: Option<String>,

    /// hypo 依赖列表（如 `@bob/utils >= 2.0.0`）。
    #[serde(default)]
    pub hypo_deps: Vec<String>,

    /// 系统依赖列表。
    #[serde(default)]
    pub system_deps: Vec<String>,
}

// ── 第二层：版本下载信息 ────────────────────────────────────

/// 某版本各平台下载信息 `hypo-package.json`（SPEC 7.4）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypoPackage {
    /// Schema 版本号。
    pub schema_version: u32,

    /// 包名。
    pub name: String,

    /// 版本号。
    pub version: String,

    /// 各平台的包下载条目。
    pub packages: Vec<HypoPackageEntry>,
}

/// 单个平台/架构的 .hypo 包下载信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypoPackageEntry {
    /// 目标平台（windows / linux / macos / all）。
    pub platform: String,

    /// 目标架构列表。
    pub arch: Vec<String>,

    /// .hypo 文件下载 URL。
    pub url: String,

    /// 文件大小（字节）。
    pub size: u64,

    /// .hypo 文件 SHA256 哈希。
    pub sha256: String,

    /// .hypo.sig 签名文件下载 URL。
    pub sig_url: String,
}

// ── 单元测试 ────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_json_roundtrip() {
        let json = r#"{
            "schema_version": 1,
            "snapshot_version": 42,
            "shards": ["a", "b"],
            "shard_hashes": {
                "a/alice.json": "sha256:abc123",
                "b/bob.json": "sha256:def456"
            },
            "official_key_fingerprints": ["A1B2C3"],
            "key_rotation": {
                "old_key_fingerprint": "F6E5D4",
                "old_key_retired_at": "2026-10-01T00:00:00Z",
                "transition_period_days": 90
            },
            "key_update_url": "https://example.com/keys/current.json",
            "mirrors": []
        }"#;

        let registry: RegistryJson = serde_json::from_str(json).expect("反序列化失败");
        assert_eq!(registry.schema_version, 1);
        assert_eq!(registry.snapshot_version, 42);
        assert_eq!(registry.shards.len(), 2);
        assert_eq!(
            registry.shard_hashes.get("a/alice.json").unwrap(),
            "sha256:abc123"
        );
        assert_eq!(registry.key_rotation.transition_period_days, 90);

        // 序列化回去不丢失关键字段
        let re_json = serde_json::to_string(&registry).expect("序列化失败");
        let re_parsed: RegistryJson = serde_json::from_str(&re_json).expect("二次反序列化失败");
        assert_eq!(re_parsed.snapshot_version, 42);
    }

    #[test]
    fn test_shard_json_roundtrip() {
        let json = r#"{
            "github_username": "alice",
            "gpg_key_fingerprints": ["A1B2C3D4E5F6"],
            "base_pkg_url": "https://alice.github.io/hypo-pkgs",
            "registered_at": "2026-01-15T08:00:00Z"
        }"#;

        let shard: ShardJson = serde_json::from_str(json).expect("反序列化失败");
        assert_eq!(shard.github_username, "alice");
        assert_eq!(shard.gpg_key_fingerprints.len(), 1);

        let re_json = serde_json::to_string(&shard).expect("序列化失败");
        let re_parsed: ShardJson = serde_json::from_str(&re_json).expect("二次反序列化失败");
        assert_eq!(re_parsed.base_pkg_url, "https://alice.github.io/hypo-pkgs");
    }

    #[test]
    fn test_hypo_index_roundtrip() {
        let json = r#"{
            "schema_version": 1,
            "owner": "alice",
            "packages": [
                {
                    "name": "my-tool",
                    "description": "A cool tool",
                    "repo": "alice/my-tool",
                    "latest_version": "1.2.3",
                    "versions": [
                        {
                            "version": "1.2.3",
                            "released_at": "2026-06-01T12:00:00Z",
                            "package_index_path": "my-tool/1.2.3/hypo-package.json",
                            "manifest_path": "my-tool/1.2.3/manifest.toml",
                            "manifest_sig_path": "my-tool/1.2.3/manifest.toml.sig",
                            "freeze": false,
                            "freeze_reason": null,
                            "rollback_version": null,
                            "hypo_deps": ["@bob/utils >= 2.0.0"],
                            "system_deps": []
                        }
                    ]
                }
            ]
        }"#;

        let index: HypoIndex = serde_json::from_str(json).expect("反序列化失败");
        assert_eq!(index.owner, "alice");
        assert_eq!(index.packages.len(), 1);
        assert_eq!(index.packages[0].versions[0].version, "1.2.3");

        let re_json = serde_json::to_string(&index).expect("序列化失败");
        let re_parsed: HypoIndex = serde_json::from_str(&re_json).expect("二次反序列化失败");
        assert_eq!(re_parsed.packages[0].latest_version, "1.2.3");
    }

    #[test]
    fn test_hypo_package_roundtrip() {
        let json = r#"{
            "schema_version": 1,
            "name": "my-tool",
            "version": "1.2.3",
            "packages": [
                {
                    "platform": "windows",
                    "arch": ["x86_64", "aarch64"],
                    "url": "https://github.com/alice/my-tool/releases/download/v1.2.3/my-tool-1.2.3-windows.hypo",
                    "size": 245678,
                    "sha256": "abc123def456",
                    "sig_url": "https://github.com/alice/my-tool/releases/download/v1.2.3/my-tool-1.2.3-windows.hypo.sig"
                }
            ]
        }"#;

        let pkg: HypoPackage = serde_json::from_str(json).expect("反序列化失败");
        assert_eq!(pkg.packages.len(), 1);
        assert_eq!(pkg.packages[0].platform, "windows");
        assert_eq!(pkg.packages[0].arch, vec!["x86_64", "aarch64"]);

        let re_json = serde_json::to_string(&pkg).expect("序列化失败");
        let re_parsed: HypoPackage = serde_json::from_str(&re_json).expect("二次反序列化失败");
        assert_eq!(re_parsed.packages[0].size, 245678);
    }
}
