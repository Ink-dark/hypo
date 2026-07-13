//! .hypo 包下载与解包。
//!
//! [`HypoPackageReader`] 封装 .hypo 文件的完整生命周期：
//! 下载（带进度条）→ 解包 → 读取 manifest。

use std::io::Write as _;
use std::path::{Path, PathBuf};

use indicatif::{ProgressBar, ProgressStyle};

use crate::error::HypoError;
use crate::package::manifest::Manifest;

/// .hypo 包读取器。
///
/// 封装已下载（或本地已有）的 .hypo 文件路径，
/// 提供解包、manifest 读取功能。
pub struct HypoPackageReader {
    /// .hypo 文件在磁盘上的路径。
    file_path: PathBuf,
}

impl HypoPackageReader {
    /// 关联一个已下载的 .hypo 文件。
    pub fn new(file_path: PathBuf) -> Self {
        Self { file_path }
    }

    /// 下载 .hypo 文件到目标路径，带进度条。
    ///
    /// 使用 `reqwest` 流式下载 + `indicatif` 进度条。
    pub async fn download(url: &str, dest: &Path) -> Result<(), HypoError> {
        let client = reqwest::Client::builder()
            .user_agent(format!("hypo/{}", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(300))
            .redirect(reqwest::redirect::Policy::limited(3))
            .build()
            .map_err(|e| HypoError::Network(format!("创建下载客户端失败: {e}")))?;

        let mut response = client
            .get(url)
            .send()
            .await
            .map_err(|e| HypoError::Network(format!("下载失败: {e}")))?;

        if !response.status().is_success() {
            return Err(HypoError::Network(format!(
                "下载返回 HTTP {}",
                response.status()
            )));
        }

        let total_size = response.content_length().unwrap_or(0);

        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{msg}\n{wide_bar} {bytes}/{total_bytes} ({bytes_per_sec}) {eta}")
                .unwrap(),
        );
        pb.set_message(format!("下载 {}", url));

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| HypoError::Network(format!("创建下载目录失败: {e}")))?;
        }

        let mut file = std::fs::File::create(dest)
            .map_err(|e| HypoError::Network(format!("创建下载文件失败: {e}")))?;

        let mut downloaded: u64 = 0;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| HypoError::Network(format!("下载数据流错误: {e}")))?
        {
            file.write_all(&chunk)
                .map_err(|e| HypoError::Network(format!("写入下载文件失败: {e}")))?;
            downloaded += chunk.len() as u64;
            pb.set_position(downloaded);
        }

        pb.finish_with_message("下载完成");

        Ok(())
    }

    /// 将 .hypo 包解压到目标目录。
    ///
    /// .hypo 文件是 ZIP 压缩包（nupkg 风格布局），使用 `zip` crate 解包。
    pub fn extract_to(&self, dest: &Path) -> Result<(), HypoError> {
        std::fs::create_dir_all(dest)
            .map_err(|e| HypoError::HashMismatch(format!("创建解包目录失败: {e}")))?;

        let file = std::fs::File::open(&self.file_path)
            .map_err(|e| HypoError::HashMismatch(format!("打开 .hypo 文件失败: {e}")))?;

        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| HypoError::HashMismatch(format!("解析 .hypo ZIP 失败: {e}")))?;

        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| HypoError::HashMismatch(format!("读取 ZIP 条目 {i} 失败: {e}")))?;

            let path = dest.join(entry.name());

            // 安全检查：拒绝路径穿越（ZIP slip）
            if !path.starts_with(dest) {
                return Err(HypoError::HashMismatch(format!(
                    "ZIP 条目路径穿越攻击: {}",
                    entry.name()
                )));
            }

            if entry.is_dir() {
                std::fs::create_dir_all(&path).map_err(|e| {
                    HypoError::HashMismatch(format!("创建目录 {} 失败: {e}", path.display()))
                })?;
            } else {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        HypoError::HashMismatch(format!(
                            "创建父目录 {} 失败: {e}",
                            parent.display()
                        ))
                    })?;
                }
                let mut out = std::fs::File::create(&path).map_err(|e| {
                    HypoError::HashMismatch(format!("创建文件 {} 失败: {e}", path.display()))
                })?;
                std::io::copy(&mut entry, &mut out).map_err(|e| {
                    HypoError::HashMismatch(format!("解压文件 {} 失败: {e}", entry.name()))
                })?;
            }
        }

        Ok(())
    }

    /// 从 .hypo 包中读取 manifest.toml（不解包全部内容）。
    pub fn read_manifest(&self) -> Result<Manifest, HypoError> {
        let file = std::fs::File::open(&self.file_path)
            .map_err(|e| HypoError::HashMismatch(format!("打开 .hypo 文件失败: {e}")))?;

        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| HypoError::HashMismatch(format!("解析 .hypo ZIP 失败: {e}")))?;

        let entry = archive
            .by_name("manifest.toml")
            .map_err(|_| HypoError::HashMismatch("包内缺少 manifest.toml".to_string()))?;

        let content = std::io::read_to_string(entry).unwrap_or_default();
        Manifest::parse(&content)
    }

    /// 列出 .hypo 包内所有文件路径。
    pub fn list_files(&self) -> Result<Vec<String>, HypoError> {
        let file = std::fs::File::open(&self.file_path)
            .map_err(|e| HypoError::HashMismatch(format!("打开 .hypo 文件失败: {e}")))?;

        let archive = zip::ZipArchive::new(file)
            .map_err(|e| HypoError::HashMismatch(format!("解析 .hypo ZIP 失败: {e}")))?;

        Ok(archive.file_names().map(|s| s.to_string()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_hypo(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hypo-test-{name}"));
        std::fs::create_dir_all(&dir).unwrap();

        let manifest_content = r#"
[package]
name = "test-pkg"
version = "0.1.0"
author = "test"
platform = "windows"
arch = ["x86_64"]

[scripts]
install = "tools/install.ps1"

[sandbox]
allowed_write_paths = ["test"]
allowed_network_egress = []

[hashes]
"tools/install.ps1" = "abc123"
"#;

        // 创建 ZIP
        let hypo_path = dir.join("test.hypo");
        let file = std::fs::File::create(&hypo_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);

        zip.start_file("manifest.toml", options).unwrap();
        zip.write_all(manifest_content.as_bytes()).unwrap();
        zip.start_file("tools/install.ps1", options).unwrap();
        zip.write_all(b"Write-Host 'installing'").unwrap();

        zip.finish().unwrap();
        hypo_path
    }

    #[test]
    fn test_extract_and_read_manifest() {
        let hypo_path = create_test_hypo("extract");
        let reader = HypoPackageReader::new(hypo_path.clone());

        let extract_dir = std::env::temp_dir().join("hypo-test-extract2");
        std::fs::create_dir_all(&extract_dir).unwrap();

        reader.extract_to(&extract_dir).expect("解包失败");

        let manifest_path = extract_dir.join("manifest.toml");
        assert!(manifest_path.exists());
        assert!(extract_dir.join("tools/install.ps1").exists());

        std::fs::remove_dir_all(&extract_dir).ok();
        std::fs::remove_dir_all(hypo_path.parent().unwrap()).ok();
    }

    #[test]
    fn test_list_files() {
        let hypo_path = create_test_hypo("list");
        let reader = HypoPackageReader::new(hypo_path.clone());
        let files = reader.list_files().expect("列出文件失败");
        assert!(files.contains(&"manifest.toml".to_string()));
        assert!(files.contains(&"tools/install.ps1".to_string()));
        std::fs::remove_dir_all(hypo_path.parent().unwrap()).ok();
    }
}
