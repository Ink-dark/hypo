//! `--from-url` 混合信任模型。
//!
//! 支持两种模式：
//! - **GitHub Pages**（`*.github.io`）：自动信任，公钥从 GitHub GPG Keys API 拉取
//! - **其他 URL**：TOFU（Trust On First Use），首次安装显示公钥指纹，
//!   用户通过 `dialoguer` 确认后缓存到 keyring

use crate::crypto::keyring;
use crate::error::HypoError;
use crate::registry::types::ShardJson;

/// 从 URL 字符串中提取主机名（host），同时拒绝含 userinfo 的 URL。
///
/// 使用 `url::Url` 进行标准化解析。
/// 若 URL 包含 userinfo（`user@host`）则返回 `None`，防止钓鱼攻击。
fn extract_host(raw_url: &str) -> Option<String> {
    use std::str::FromStr;
    let url_str = if raw_url.contains("://") {
        raw_url.to_string()
    } else {
        format!("https://{raw_url}")
    };
    let parsed = url::Url::from_str(&url_str).ok()?;
    // 拒绝含 userinfo 的 URL（如 evil.com@alice.github.io）
    if parsed.username() != "" || parsed.password().is_some() {
        return None;
    }
    parsed.host_str().map(|h| h.to_lowercase())
}

/// 检查域名是否为 GitHub Pages 域。
fn is_github_pages_host(host: &str) -> bool {
    host == "github.io" || host.ends_with(".github.io")
}

/// 判断 URL 是否为 GitHub Pages 域名。
///
/// 使用标准 URL 解析器提取主机名后再判断，防止 `evil.com@alice.github.io`
/// 类型的 userinfo 注入攻击。
pub fn is_github_pages_url(url: &str) -> bool {
    extract_host(url).is_some_and(|h| is_github_pages_host(&h))
}

/// 从 GitHub Pages URL 中提取 GitHub username。
///
/// 例如 `https://alice.github.io/hypo-pkgs` → `alice`
pub fn extract_github_username(url: &str) -> Option<String> {
    let host = extract_host(url)?;
    if !is_github_pages_host(&host) {
        return None;
    }
    // 从 host 中提取子域名部分：alice.github.io → alice
    host.strip_suffix(".github.io")
        .filter(|s| !s.is_empty() && s != &"github.io")
        .map(|s| s.to_string())
}

/// 验证 owner 名符合 GitHub username 规范。
///
/// 规则：仅字母数字与连字符，长度 1-39。
pub fn validate_owner(owner: &str) -> Result<(), HypoError> {
    if owner.is_empty() || owner.len() > 39 {
        return Err(HypoError::RegistryNotFound(format!(
            "无效的 owner 名称：{owner}"
        )));
    }
    if !owner.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(HypoError::RegistryNotFound(format!(
            "owner 名称包含非法字符：{owner}"
        )));
    }
    Ok(())
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
        // userinfo 注入攻击应被拒绝
        assert!(!is_github_pages_url(
            "https://evil.com@alice.github.io/hypo-pkgs"
        ));
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
        // userinfo 注入攻击
        assert_eq!(
            extract_github_username("https://evil.com@alice.github.io"),
            None
        );
    }

    #[test]
    fn test_validate_owner() {
        assert!(validate_owner("alice").is_ok());
        assert!(validate_owner("my-org").is_ok());
        assert!(validate_owner("test_user").is_err()); // 下划线
        assert!(validate_owner("../evil").is_err()); // 路径穿越
        assert!(validate_owner("").is_err());
    }
}
