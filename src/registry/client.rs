//! Registry HTTP 客户端。
//!
//! 负责拉取两层 registry 数据：
//! 1. 官方目录：`registry.json` + `registry.sig` + 分片
//! 2. 开发者 registry：`hypo-index.json` + `hypo-package.json` + `manifest.toml`

use reqwest::Client;

use crate::constants::OFFICIAL_DIR_BASE_URL;
use crate::error::HypoError;
use crate::registry::cache;
use crate::registry::types::*;

/// 创建带安全策略的 HTTP 客户端。
///
/// - User-Agent：`hypo/{version}`
/// - 超时：30 秒
/// - 重定向：最多 3 次（防止无限重定向攻击）
/// - TLS：rustls 默认验证
fn http_client() -> Result<Client, HypoError> {
    Client::builder()
        .user_agent(format!("hypo/{}", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .map_err(|e| HypoError::Network(format!("创建 HTTP 客户端失败: {e}")))
}

// ── 第一层：官方目录 ──────────────────────────────────────────────

/// 拉取官方目录 `registry.json` 及其签名 `registry.sig`。
///
/// 返回 `(RegistryJson, 原始 JSON 字节)`。
/// 签名验证由调用方负责（需要加载根证书后调用 [`verify::verify_registry_sig`]）。
pub async fn fetch_registry_json() -> Result<(RegistryJson, Vec<u8>), HypoError> {
    let client = http_client()?;
    let url = format!("{OFFICIAL_DIR_BASE_URL}/registry.json");

    let request = client.get(&url);
    let request = cache::apply_cache_headers(request, &url);
    let response = request
        .send()
        .await
        .map_err(|e| HypoError::Network(format!("拉取 registry.json 失败: {e}")))?;

    if !response.status().is_success() {
        return Err(HypoError::Network(format!(
            "registry.json 返回 HTTP {}",
            response.status()
        )));
    }

    cache::update_cache_from_response(&url, &response);

    let raw_json = response
        .bytes()
        .await
        .map_err(|e| HypoError::Network(format!("读取 registry.json 响应体失败: {e}")))?;

    let registry: RegistryJson = serde_json::from_slice(&raw_json)
        .map_err(|e| HypoError::Network(format!("registry.json 解析失败: {e}")))?;

    Ok((registry, raw_json.to_vec()))
}

/// 拉取 `registry.sig` 签名文件。
pub async fn fetch_registry_sig() -> Result<Vec<u8>, HypoError> {
    let client = http_client()?;
    let url = format!("{OFFICIAL_DIR_BASE_URL}/registry.sig");
    fetch_raw(&client, &url).await
}

/// 拉取 `registry.sig.old` 旧密钥签名文件（过渡期）。
pub async fn fetch_registry_sig_old() -> Result<Vec<u8>, HypoError> {
    let client = http_client()?;
    let url = format!("{OFFICIAL_DIR_BASE_URL}/registry.sig.old");
    fetch_raw(&client, &url).await
}

/// 按 owner 首字母拉取开发者分片，并校验 SHA256。
///
/// 从 `registry.json` 的 `shard_hashes` 中获取期望的 SHA256，
/// 拉取后计算实际 SHA256 对比，不匹配返回 [`HypoError::HashMismatch`]（退出码 11）。
pub async fn fetch_shard(registry: &RegistryJson, owner: &str) -> Result<ShardJson, HypoError> {
    // 安全校验：owner 必须符合 GitHub username 规范
    crate::registry::trust::validate_owner(owner)?;

    let client = http_client()?;

    // 按首字母定位分片：a/alice.json
    let first_char = owner
        .chars()
        .next()
        .unwrap_or('_')
        .to_lowercase()
        .next()
        .unwrap_or('_');
    let shard_path = format!("{first_char}/{owner}.json");
    let url = format!("{OFFICIAL_DIR_BASE_URL}/{shard_path}");

    let request = client.get(&url);
    let request = cache::apply_cache_headers(request, &url);
    let response = request
        .send()
        .await
        .map_err(|e| HypoError::Network(format!("拉取分片 {shard_path} 失败: {e}")))?;

    if !response.status().is_success() {
        return Err(HypoError::RegistryNotFound(format!(
            "开发者 {owner} 的分片不存在（HTTP {}）",
            response.status()
        )));
    }

    cache::update_cache_from_response(&url, &response);

    let raw = response
        .bytes()
        .await
        .map_err(|e| HypoError::Network(format!("读取分片响应体失败: {e}")))?;

    // SHA256 校验
    verify_shard_hash(&raw, &shard_path, registry)?;

    let shard: ShardJson = serde_json::from_slice(&raw)
        .map_err(|e| HypoError::Network(format!("分片 JSON 解析失败: {e}")))?;

    Ok(shard)
}

/// 校验分片文件的 SHA256 哈希。
fn verify_shard_hash(
    raw: &[u8],
    shard_path: &str,
    registry: &RegistryJson,
) -> Result<(), HypoError> {
    use sha2::{Digest, Sha256};

    let expected = registry.shard_hashes.get(shard_path).ok_or_else(|| {
        HypoError::HashMismatch(format!(
            "分片 {shard_path} 的 SHA256 哈希未在 registry.json 中登记，拒绝信任"
        ))
    })?;

    let actual = format!("{:x}", Sha256::digest(raw));
    // expected 格式为 "sha256:abc123..."，支持两种格式
    let expected_hash = expected.strip_prefix("sha256:").unwrap_or(expected);

    if !actual.eq_ignore_ascii_case(expected_hash) {
        return Err(HypoError::HashMismatch(format!(
            "分片 {shard_path} SHA256 不匹配：期望 {expected_hash}，实际 {actual}"
        )));
    }

    Ok(())
}

// ── 第二层：开发者 registry ──────────────────────────────────────

/// 拉取开发者的 `hypo-index.json`。
pub async fn fetch_hypo_index(base_pkg_url: &str) -> Result<HypoIndex, HypoError> {
    let client = http_client()?;
    let url = format!("{base_pkg_url}/hypo-index.json");
    let raw = fetch_raw(&client, &url).await?;

    serde_json::from_slice(&raw)
        .map_err(|e| HypoError::Network(format!("hypo-index.json 解析失败: {e}")))
}

/// 拉取指定版本包的 `hypo-package.json`。
pub async fn fetch_hypo_package(
    base_pkg_url: &str,
    pkg: &str,
    ver: &str,
) -> Result<HypoPackage, HypoError> {
    let client = http_client()?;
    let url = format!("{base_pkg_url}/{pkg}/{ver}/hypo-package.json");
    let raw = fetch_raw(&client, &url).await?;

    serde_json::from_slice(&raw)
        .map_err(|e| HypoError::Network(format!("hypo-package.json 解析失败: {e}")))
}

/// 拉取 `manifest.toml` 及其签名 `manifest.toml.sig`。
///
/// 返回 `(manifest_bytes, sig_bytes)`。
pub async fn fetch_manifest(
    base_pkg_url: &str,
    pkg: &str,
    ver: &str,
) -> Result<(Vec<u8>, Vec<u8>), HypoError> {
    let client = http_client()?;
    let manifest_url = format!("{base_pkg_url}/{pkg}/{ver}/manifest.toml");
    let sig_url = format!("{base_pkg_url}/{pkg}/{ver}/manifest.toml.sig");

    let manifest = fetch_raw(&client, &manifest_url).await?;
    let sig = fetch_raw(&client, &sig_url).await?;

    Ok((manifest, sig))
}

// ── 内部工具 ─────────────────────────────────────────────────────

/// 执行 GET 请求并返回响应体字节。
async fn fetch_raw(client: &Client, url: &str) -> Result<Vec<u8>, HypoError> {
    let request = client.get(url);
    let request = cache::apply_cache_headers(request, url);
    let response = request
        .send()
        .await
        .map_err(|e| HypoError::Network(format!("请求 {url} 失败: {e}")))?;

    if !response.status().is_success() {
        return Err(HypoError::Network(format!(
            "{url} 返回 HTTP {}",
            response.status()
        )));
    }

    cache::update_cache_from_response(url, &response);

    response
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| HypoError::Network(format!("读取 {url} 响应体失败: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_shard_hash_missing_key_rejected() {
        // 哈希表中无对应条目 → 应返回错误
        let registry = RegistryJson {
            schema_version: 1,
            snapshot_version: 1,
            shards: vec![],
            shard_hashes: std::collections::HashMap::new(),
            official_key_fingerprints: vec![],
            key_rotation: KeyRotation {
                old_key_fingerprint: "".into(),
                old_key_retired_at: "".into(),
                transition_period_days: 90,
            },
            key_update_url: "".into(),
            mirrors: vec![],
        };
        let result = verify_shard_hash(b"dummy", "a/alice.json", &registry);
        assert!(result.is_err(), "缺失哈希条目应返回错误");
    }

    #[test]
    fn test_verify_shard_hash_mismatch() {
        let mut shard_hashes = std::collections::HashMap::new();
        shard_hashes.insert("a/alice.json".into(), "sha256:deadbeef".into());
        let registry = RegistryJson {
            schema_version: 1,
            snapshot_version: 1,
            shards: vec![],
            shard_hashes,
            official_key_fingerprints: vec![],
            key_rotation: KeyRotation {
                old_key_fingerprint: "".into(),
                old_key_retired_at: "".into(),
                transition_period_days: 90,
            },
            key_update_url: "".into(),
            mirrors: vec![],
        };
        let result = verify_shard_hash(b"test data", "a/alice.json", &registry);
        assert!(result.is_err());
    }
}
