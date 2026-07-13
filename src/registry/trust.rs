//! `--from-url` 混合信任模型。
//!
//! ## 信任原则
//!
//! 开发者身份由 **分片中声明的 GPG 指纹**（[`ShardJson::gpg_key_fingerprints`]）确立。
//! 托管平台（GitHub Pages / Vercel / Cloudflare Pages / 自建站）仅影响 **公钥获取方式**，
//! 不影响信任判定。
//!
//! ## 公钥获取优先级
//!
//! 1. 本地 keyring 缓存（`~/.hypo/keyring/{fingerprint}.asc`）
//! 2. GitHub Pages → GitHub GPG Keys API（自动）
//! 3. `{base_pkg_url}/gpg-key.asc` well-known 路径（自动）
//! 4. TOFU：显示指纹，用户通过 `dialoguer` 确认后缓存

use crate::crypto::keyring;
use crate::error::HypoError;
use crate::registry::types::ShardJson;

/// 从 URL 字符串中提取主机名（host），同时拒绝含 userinfo 的 URL。
fn extract_host(raw_url: &str) -> Option<String> {
    use std::str::FromStr;
    let url_str = if raw_url.contains("://") {
        raw_url.to_string()
    } else {
        format!("https://{raw_url}")
    };
    let parsed = url::Url::from_str(&url_str).ok()?;
    // 拒绝含 userinfo 的 URL（钓鱼攻击）
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return None;
    }
    parsed.host_str().map(|h| h.to_lowercase())
}

// ── 平台检测（仅影响公钥获取方式，不影响信任） ──────────────────

/// 检查域名是否为 GitHub Pages 域。
fn is_github_pages_host(host: &str) -> bool {
    host == "github.io" || host.ends_with(".github.io")
}

/// 判断 URL 是否托管在 GitHub Pages。
///
/// 仅用于决定是否走 GitHub API 快捷获取公钥，**不用于信任判定**。
pub fn is_github_pages_url(url: &str) -> bool {
    extract_host(url).is_some_and(|h| is_github_pages_host(&h))
}

/// 从 GitHub Pages URL 中提取 GitHub username。
pub fn extract_github_username(url: &str) -> Option<String> {
    let host = extract_host(url)?;
    if !is_github_pages_host(&host) {
        return None;
    }
    host.strip_suffix(".github.io")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

// ── owner 名校验 ──────────────────────────────────────────────

/// 验证 owner 名符合 GitHub username 规范（也适用于其他平台标识符）。
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

// ── 公钥解析 ──────────────────────────────────────────────────

/// 尝试从多个来源解析开发者的 GPG 公钥，返回证书。
///
/// 优先级：
/// 1. 本地 keyring 缓存
/// 2. GitHub Pages → GitHub GPG Keys API
/// 3. `{base_pkg_url}/gpg-key.asc` well-known 路径
/// 4. 以上都失败 → 返回 `GpgNoPubkey`，由上层触发 TOFU
pub async fn resolve_developer_cert(
    fingerprints: &[String],
    base_pkg_url: &str,
) -> Result<sequoia_openpgp::Cert, HypoError> {
    use sequoia_openpgp::parse::Parse;

    // 取第一个指纹（开发者通常只有一个签名密钥）
    let fingerprint = fingerprints
        .first()
        .ok_or_else(|| HypoError::GpgNoPubkey("分片中未声明 GPG 指纹".to_string()))?;

    // 优先级 1：本地 keyring
    if let Some(cert) = keyring::load_cert(fingerprint) {
        return Ok(cert);
    }

    // 优先级 2：GitHub Pages → GitHub API
    if is_github_pages_url(base_pkg_url) {
        if let Some(username) = extract_github_username(base_pkg_url) {
            let gh_fingerprints = crate::crypto::github::fetch_gpg_keys(&username).await?;
            for fp in &gh_fingerprints {
                if fp.eq_ignore_ascii_case(fingerprint) {
                    // 从 keyring 重新加载（fetch_gpg_keys 只返回指纹，
                    // 实际下载和缓存由 github.rs 的改进版完成）
                    if let Some(cert) = keyring::load_cert(fingerprint) {
                        return Ok(cert);
                    }
                }
            }
        }
    }

    // 优先级 3：well-known URL
    let well_known_url = format!("{base_pkg_url}/gpg-key.asc");
    if let Ok(cert_bytes) = fetch_raw(&well_known_url).await {
        if let Ok(cert) = sequoia_openpgp::Cert::from_bytes(&cert_bytes) {
            let _ = keyring::save_cert(&cert);
            return Ok(cert);
        }
    }

    // 优先级 4：无法自动获取
    Err(HypoError::GpgNoPubkey(format!(
        "公钥 {fingerprint} 未在本地缓存中找到。\
         请将 GPG 公钥导出到 {base_pkg_url}/gpg-key.asc，\
         或通过 hypo install --from-url 的 TOFU 流程手动信任。"
    )))
}

/// HTTP GET 请求，返回响应体字节。
async fn fetch_raw(url: &str) -> Result<Vec<u8>, HypoError> {
    let client = reqwest::Client::builder()
        .user_agent(format!("hypo/{}", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .map_err(|e| HypoError::Network(format!("创建 HTTP 客户端失败: {e}")))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| HypoError::Network(format!("请求 {url} 失败: {e}")))?;

    if !response.status().is_success() {
        return Err(HypoError::Network(format!(
            "{url} 返回 HTTP {}",
            response.status()
        )));
    }

    response
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| HypoError::Network(format!("读取 {url} 响应体失败: {e}")))
}

// ── TOFU 信任 ─────────────────────────────────────────────────

/// 对非自动可获取公钥的站点建立 TOFU 信任。
///
/// 首次安装时显示公钥指纹，用户通过 `dialoguer` 确认后缓存到 keyring。
///
/// 注意：实际的用户交互由命令层（Step 8）通过 `dialoguer` 完成，
/// 此处仅检查是否已有缓存。
pub async fn tofu_trust(fingerprint: &str) -> Result<(), HypoError> {
    if keyring::load_cert(fingerprint).is_some() {
        return Ok(());
    }

    Err(HypoError::SignatureVerification(format!(
        "TOFU 模式：公钥指纹 {fingerprint} 尚未被信任。\n\
         请确认这是开发者的正确指纹后，通过交互式安装流程缓存该公钥。"
    )))
}

// ── 分片信任 ──────────────────────────────────────────────────

/// 从分片信息为开发者建立信任。
///
/// 检查分片中每个指纹是否有对应缓存证书。
/// 缺失时尝试自动解析（GitHub API / well-known URL），
/// 仍缺失则标记需要 TOFU 确认。
pub async fn trust_from_shard(shard: &ShardJson) -> Result<(), HypoError> {
    let mut missing = Vec::new();
    for fp in &shard.gpg_key_fingerprints {
        if keyring::load_cert(fp).is_none() {
            missing.push(fp.clone());
        }
    }

    if !missing.is_empty() {
        // 尝试自动解析
        match resolve_developer_cert(&missing, &shard.base_pkg_url).await {
            Ok(_cert) => return Ok(()),
            Err(e) => {
                return Err(HypoError::GpgNoPubkey(format!(
                    "开发者 {} 的公钥无法自动获取：{e}。\n\
                     请确保开发者已在 {}/gpg-key.asc 发布 GPG 公钥，\
                     或通过 TOFU 流程手动信任。",
                    shard.github_username, shard.base_pkg_url
                )));
            }
        }
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
        assert!(!is_github_pages_url("https://my-tool.vercel.app"));
        assert!(!is_github_pages_url("https://hypo.example.org"));
        // userinfo 注入攻击
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
        assert_eq!(extract_github_username("https://my-app.vercel.app"), None);
    }

    #[test]
    fn test_validate_owner() {
        assert!(validate_owner("alice").is_ok());
        assert!(validate_owner("my-org").is_ok());
        assert!(validate_owner("vercel-user").is_ok());
        assert!(validate_owner("test_user").is_err());
        assert!(validate_owner("../evil").is_err());
        assert!(validate_owner("").is_err());
    }
}
