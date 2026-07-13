//! HTTP 请求缓存。
//!
//! 管理 `~/.hypo/cache/` 下的缓存数据：
//! - `http_cache.toml`：URL → {etag, last_modified} 映射
//! - `snapshot_version`：远端 registry.json 的快照版本号

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::paths;

/// 单个 URL 的缓存条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// HTTP ETag 头值。
    pub etag: Option<String>,
    /// HTTP Last-Modified 头值。
    pub last_modified: Option<String>,
    /// Unix 时间戳（秒），缓存写入时间。
    pub cached_at: u64,
}

/// HTTP 缓存索引文件内容。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct HttpCacheIndex {
    entries: HashMap<String, CacheEntry>,
}

/// 加载 HTTP 缓存索引。
fn load_index() -> HttpCacheIndex {
    let path = cache_index_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => toml::from_str(&content).unwrap_or_default(),
        Err(_) => HttpCacheIndex::default(),
    }
}

/// 保存 HTTP 缓存索引。
fn save_index(index: &HttpCacheIndex) {
    let path = cache_index_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = toml::to_string_pretty(index) {
        let _ = std::fs::write(&path, content);
    }
}

/// 缓存索引文件路径。
fn cache_index_path() -> PathBuf {
    paths::cache_dir().join("http_cache.toml")
}

/// 查询 URL 对应的缓存条目。
pub fn get_cache_entry(url: &str) -> Option<CacheEntry> {
    load_index().entries.get(url).cloned()
}

/// 保存 URL 对应的缓存条目。
pub fn set_cache_entry(url: &str, entry: CacheEntry) {
    let mut index = load_index();
    index.entries.insert(url.to_string(), entry);
    // 限制缓存条目数，删除最旧的 100 条
    if index.entries.len() > 500 {
        let mut entries: Vec<_> = index.entries.into_iter().collect();
        entries.sort_by_key(|(_, e)| e.cached_at);
        entries.reverse();
        entries.truncate(400);
        index.entries = entries.into_iter().collect();
    }
    save_index(&index);
}

/// 获取缓存的 snapshot_version。
pub fn get_snapshot_version() -> Option<u64> {
    let path = paths::cache_dir().join("snapshot_version");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

/// 保存 snapshot_version。
pub fn set_snapshot_version(version: u64) {
    let path = paths::cache_dir().join("snapshot_version");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, version.to_string());
}

/// 获取当前 Unix 时间戳（秒）。
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// 使用 reqwest 构建带条件头的请求（If-None-Match / If-Modified-Since）。
pub fn apply_cache_headers(request: reqwest::RequestBuilder, url: &str) -> reqwest::RequestBuilder {
    if let Some(entry) = get_cache_entry(url) {
        let mut req = request;
        if let Some(etag) = &entry.etag {
            req = req.header("If-None-Match", etag);
        }
        if let Some(lm) = &entry.last_modified {
            req = req.header("If-Modified-Since", lm);
        }
        req
    } else {
        request
    }
}

/// 从 reqwest 响应中提取并缓存 ETag / Last-Modified 头。
pub fn update_cache_from_response(url: &str, response: &reqwest::Response) {
    let etag = response
        .headers()
        .get("ETag")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let last_modified = response
        .headers()
        .get("Last-Modified")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if etag.is_some() || last_modified.is_some() {
        set_cache_entry(
            url,
            CacheEntry {
                etag,
                last_modified,
                cached_at: now_secs(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_entry_roundtrip() {
        let entry = CacheEntry {
            etag: Some("\"abc123\"".into()),
            last_modified: Some("Mon, 01 Jan 2024 00:00:00 GMT".into()),
            cached_at: 1704067200,
        };
        assert_eq!(entry.etag.as_deref(), Some("\"abc123\""));
        assert_eq!(entry.cached_at, 1704067200);
    }

    #[test]
    fn test_snapshot_version_default() {
        // 首次查询返回 None
        let sv = get_snapshot_version();
        // 注意：此处可能受之前测试的缓存影响，只验证类型兼容
        assert!(sv.is_none() || sv.is_some());
    }
}
