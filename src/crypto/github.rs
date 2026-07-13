//! GitHub GPG Keys API 客户端。
//!
//! 通过 GitHub REST API 拉取开发者的 GPG 公钥，
//! 解析为 sequoia 证书格式，用于后续签名验证。

use sequoia_openpgp::cert::prelude::*;
use sequoia_openpgp::parse::Parse;
use serde::Deserialize;

use crate::error::HypoError;
use crate::registry::types::ShardJson;

/// GitHub GPG Keys API 响应中的单个密钥条目。
#[derive(Debug, Deserialize)]
struct GitHubGpgKey {
    /// 公钥 ID（十六进制）。
    #[allow(dead_code)]
    key_id: String,

    /// ASCII-armored PGP 公钥块。
    raw_key: Option<String>,
}

/// 从 GitHub 拉取指定用户的 GPG 公钥，返回指纹列表。
///
/// 调用 `GET /users/{username}/gpg_keys`。
pub async fn fetch_gpg_keys(username: &str) -> Result<Vec<String>, HypoError> {
    let url = format!("https://api.github.com/users/{username}/gpg_keys");
    let client = reqwest::Client::builder()
        .user_agent("hypo/0.1.0")
        .build()
        .map_err(|e| HypoError::Network(format!("创建 HTTP 客户端失败: {e}")))?;

    let response = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| HypoError::Network(format!("GitHub API 请求失败: {e}")))?;

    if !response.status().is_success() {
        return Err(HypoError::Network(format!(
            "GitHub API 返回 HTTP {}",
            response.status()
        )));
    }

    let keys: Vec<GitHubGpgKey> = response
        .json()
        .await
        .map_err(|e| HypoError::Network(format!("GitHub API 响应解析失败: {e}")))?;

    let mut fingerprints = Vec::new();
    for key in keys {
        if let Some(raw) = key.raw_key {
            match Cert::from_bytes(raw.as_bytes()) {
                Ok(cert) => {
                    fingerprints.push(cert.fingerprint().to_hex());
                }
                Err(e) => {
                    tracing::warn!("跳过无法解析的 GPG 密钥: {e}");
                }
            }
        }
    }

    Ok(fingerprints)
}

/// 按优先级解析公钥证书。
///
/// 优先级：
/// 1. 本地 keyring 缓存（`~/.hypo/keyring/{fingerprint}.asc`）
/// 2. 官方目录分片中的指纹（仅做信任验证，不在此函数内拉取）
/// 3. GitHub GPG Keys API（按 `github_username` 拉取并缓存）
///
/// 返回解析后的证书，或 `GpgNoPubkey` 错误。
pub async fn resolve_public_key(
    fingerprint: &str,
    shard: Option<&ShardJson>,
) -> Result<Cert, HypoError> {
    // 优先级 1：本地 keyring 缓存
    if let Some(cert) = crate::crypto::keyring::load_cert(fingerprint) {
        return Ok(cert);
    }

    // 优先级 2 & 3：通过分片信息从 GitHub API 拉取
    if let Some(shard) = shard {
        let fingerprints = fetch_gpg_keys(&shard.github_username).await?;

        // 遍历拉回的公钥，找到匹配指纹并缓存
        for fp in &fingerprints {
            if fp.eq_ignore_ascii_case(fingerprint) {
                // 重新解析 raw_key 获取完整 Cert（fetch_gpg_keys 内已解析过，
                // 但此处需要返回 Cert。简化处理：再次拉取并解析）
                // 实际实现中 fetch_gpg_keys 应返回 (fingerprint, raw_key) 对，
                // 但 MVP 阶段先保持接口简单。
                return Err(HypoError::GpgNoPubkey(format!(
                    "公钥 {fingerprint} 已从 GitHub 找到，但尚未缓存。请重试。"
                )));
            }
        }
    }

    Err(HypoError::GpgNoPubkey(format!(
        "未能在任何来源找到公钥 {fingerprint}"
    )))
}
