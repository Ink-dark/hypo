//! `--from-url` 混合信任模型。
//!
//! 支持两种模式：
//! - **GitHub Pages**（`*.github.io`）：自动信任，公钥从 GitHub GPG Keys API 拉取
//! - **其他 URL**：TOFU（Trust On First Use），首次安装显示公钥指纹，
//!   用户通过 `dialoguer` 确认后缓存到 keyring

use crate::crypto::keyring;
use crate::error::HypoError;
use crate::registry::types::ShardJson;

/// 判断 URL 是否为 GitHub Pages 域名。
pub fn is_github_pages_url(url: &str) -> bool {
    // 提取域名部分，检查是否以 .github.io 结尾或等于 github.io
    let host = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or("");
    host == "github.io" || host.ends_with(".github.io")
}

/// 从 GitHub Pages URL 中提取 GitHub username。
///
/// 例如 `https://alice.github.io/hypo-pkgs` → `alice`
pub fn extract_github_username(url: &str) -> Option<String> {
    // 匹配 *.github.io 子域名
    if !is_github_pages_url(url) {
        return None;
    }
    let url = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    url.split(".github.io").next().map(|s| s.to_string())
}

/// 对非 GitHub Pages URL 建立 TOFU 信任。
///
/// 显示公钥指纹，等待用户确认后缓存到本地 keyring。
///
/// 注意：当前为 MVP 骨架，实际实现需在 Step 8 配合 CLI
/// 根据 manifest 校验结果调用此函数。
pub async fn tofu_trust(fingerprint: &str) -> Result<(), HypoError> {
    // MVP 阶段：若本地 keyring 已有该指纹，视为已信任
    if keyring::load_cert(fingerprint).is_some() {
        return Ok(());
    }

    // 需要用户交互确认 — 但 MVP 的 trust 模块先返回需要确认的状态
    // 实际交互由命令层（Step 8）通过 dialoguer 完成
    Err(HypoError::SignatureVerification(format!(
        "TOFU 模式：公钥指纹 {fingerprint} 尚未被信任。请在安装命令中确认。"
    )))
}

/// 为开发者建立信任（从分片信息）。
///
/// 尝试从本地 keyring 加载证书，若无则标记需要从 GitHub API 拉取。
pub fn trust_from_shard(shard: &ShardJson) -> Result<(), HypoError> {
    // 检查分片中每个指纹是否有对应缓存证书
    let mut missing = Vec::new();
    for fp in &shard.gpg_key_fingerprints {
        if keyring::load_cert(fp).is_none() {
            missing.push(fp.clone());
        }
    }

    if !missing.is_empty() {
        return Err(HypoError::GpgNoPubkey(format!(
            "开发者 {} 的公钥尚未缓存：{:?}。请先通过 GitHub API 拉取。",
            shard.github_username, missing
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_github_pages_url() {
        assert!(is_github_pages_url("https://alice.github.io/hypo-pkgs"));
        assert!(is_github_pages_url("http://bob.github.io"));
        assert!(!is_github_pages_url("https://example.com/hypo"));
        assert!(!is_github_pages_url("https://github.io.evil.com"));
    }

    #[test]
    fn test_extract_github_username() {
        assert_eq!(
            extract_github_username("https://alice.github.io/hypo-pkgs"),
            Some("alice".into())
        );
        assert_eq!(
            extract_github_username("https://my-org.github.io"),
            Some("my-org".into())
        );
        assert_eq!(extract_github_username("https://example.com"), None);
    }
}
