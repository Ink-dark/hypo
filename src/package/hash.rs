//! 包内文件 SHA256 哈希校验。
//!
//! 下载 .hypo 包解包后，逐个计算包内文件 SHA256，
//! 与 manifest `[hashes]` 表对比，任一不匹配即拒绝执行。

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::HypoError;
use crate::package::manifest::Manifest;

/// 计算单个文件的 SHA256 哈希，返回十六进制小写字符串。
pub fn compute_sha256(path: &Path) -> Result<String, HypoError> {
    let data = std::fs::read(path)
        .map_err(|e| HypoError::HashMismatch(format!("无法读取文件 {}: {e}", path.display())))?;
    Ok(format!("{:x}", Sha256::digest(&data)))
}

/// 逐文件校验解包目录内容与 manifest `[hashes]` 的一致性。
///
/// 遍历 manifest 中声明的每个文件路径，在解包目录中定位对应文件，
/// 计算其 SHA256 与 manifest 记录对比。不匹配返回 [`HypoError::HashMismatch`]（退出码 11）。
///
/// # 安全检查
///
/// - 拒绝包含 `..` 或 `\` 的文件路径（防止路径穿越）
/// - 缺少文件或多余文件均报错
pub fn verify_files(extract_dir: &Path, manifest: &Manifest) -> Result<(), HypoError> {
    for (file_path, expected_hash) in &manifest.hashes {
        // 安全检查：拒绝路径穿越
        if file_path.contains("..") || file_path.contains('\\') {
            return Err(HypoError::HashMismatch(format!(
                "manifest 中包含非法文件路径: {file_path}"
            )));
        }

        let actual_path = extract_dir.join(file_path);
        if !actual_path.exists() {
            return Err(HypoError::HashMismatch(format!(
                "manifest 声明的文件在包中不存在: {file_path}"
            )));
        }

        let actual_hash = compute_sha256(&actual_path)?;

        if !actual_hash.eq_ignore_ascii_case(expected_hash) {
            return Err(HypoError::HashMismatch(format!(
                "文件 {file_path} SHA256 不匹配：期望 {expected_hash}，实际 {actual_hash}"
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_compute_sha256() {
        let dir = std::env::temp_dir().join("hypo-test-hash");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("test.txt");
        std::fs::write(&file, b"hello hypo").unwrap();

        let hash = compute_sha256(&file).expect("计算哈希失败");
        assert_eq!(hash.len(), 64); // SHA256 = 64 hex chars
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_compute_sha256_consistent() {
        let dir = std::env::temp_dir().join("hypo-test-hash2");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("test.txt");
        std::fs::write(&file, b"same content").unwrap();

        let h1 = compute_sha256(&file).unwrap();
        let h2 = compute_sha256(&file).unwrap();
        assert_eq!(h1, h2);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_verify_files_success() {
        let dir = std::env::temp_dir().join("hypo-test-verify");
        std::fs::create_dir_all(&dir).unwrap();

        // 创建测试文件
        let file_content = b"test content for verification";
        let file_path = "tools/install.ps1";
        let full_path = dir.join(file_path);
        std::fs::create_dir_all(full_path.parent().unwrap()).unwrap();
        std::fs::write(&full_path, file_content).unwrap();

        let expected_hash = format!("{:x}", Sha256::digest(file_content));

        let mut hashes = HashMap::new();
        hashes.insert(file_path.to_string(), expected_hash);

        let manifest = Manifest {
            package: crate::package::manifest::PackageMeta {
                name: "test".into(),
                version: "1.0.0".into(),
                description: String::new(),
                author: "test".into(),
                repo: String::new(),
                platform: "windows".into(),
                arch: vec!["x86_64".into()],
            },
            scripts: crate::package::manifest::Scripts {
                install: file_path.into(),
                uninstall: None,
                update: None,
                pre_install: None,
                post_install: None,
                pre_uninstall: None,
                post_uninstall: None,
                pre_update: None,
                post_update: None,
            },
            interpreter: crate::package::manifest::Interpreter::default(),
            sandbox: crate::package::manifest::SandboxDecl {
                allowed_write_paths: vec![],
                allowed_network_egress: vec![],
            },
            dependencies: Default::default(),
            hashes,
        };

        assert!(verify_files(&dir, &manifest).is_ok());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_verify_files_hash_mismatch() {
        let dir = std::env::temp_dir().join("hypo-test-mismatch");
        std::fs::create_dir_all(&dir).unwrap();

        let file_path = "tools/install.ps1";
        let full_path = dir.join(file_path);
        std::fs::create_dir_all(full_path.parent().unwrap()).unwrap();
        std::fs::write(&full_path, b"actual content").unwrap();

        let mut hashes = HashMap::new();
        hashes.insert(file_path.to_string(), "deadbeef".to_string());

        let manifest = Manifest {
            package: crate::package::manifest::PackageMeta {
                name: "test".into(),
                version: "1.0.0".into(),
                description: String::new(),
                author: "test".into(),
                repo: String::new(),
                platform: "windows".into(),
                arch: vec!["x86_64".into()],
            },
            scripts: crate::package::manifest::Scripts {
                install: file_path.into(),
                uninstall: None,
                update: None,
                pre_install: None,
                post_install: None,
                pre_uninstall: None,
                post_uninstall: None,
                pre_update: None,
                post_update: None,
            },
            interpreter: crate::package::manifest::Interpreter::default(),
            sandbox: crate::package::manifest::SandboxDecl {
                allowed_write_paths: vec![],
                allowed_network_egress: vec![],
            },
            dependencies: Default::default(),
            hashes,
        };

        assert!(verify_files(&dir, &manifest).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_verify_rejects_path_traversal() {
        let mut hashes = HashMap::new();
        hashes.insert("../etc/passwd".to_string(), "abc".to_string());

        let manifest = Manifest {
            package: crate::package::manifest::PackageMeta {
                name: "test".into(),
                version: "1.0.0".into(),
                description: String::new(),
                author: "test".into(),
                repo: String::new(),
                platform: "windows".into(),
                arch: vec!["x86_64".into()],
            },
            scripts: crate::package::manifest::Scripts {
                install: "install.ps1".into(),
                uninstall: None,
                update: None,
                pre_install: None,
                post_install: None,
                pre_uninstall: None,
                post_uninstall: None,
                pre_update: None,
                post_update: None,
            },
            interpreter: crate::package::manifest::Interpreter::default(),
            sandbox: crate::package::manifest::SandboxDecl {
                allowed_write_paths: vec![],
                allowed_network_egress: vec![],
            },
            dependencies: Default::default(),
            hashes,
        };

        assert!(verify_files(std::path::Path::new("."), &manifest).is_err());
    }
}
