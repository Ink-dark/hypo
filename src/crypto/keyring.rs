//! 本地 keyring 缓存。
//!
//! 证书以 armored ASCII 格式存储在 `~/.hypo/keyring/{fingerprint}.asc`，
//! 提供保存、加载、列表操作。
//!
//! 公钥获取优先级：本地 keyring → 官方目录分片 → GitHub API。

use sequoia_openpgp::cert::prelude::*;
use sequoia_openpgp::parse::Parse;
use sequoia_openpgp::serialize::Serialize;

use crate::error::HypoError;
use crate::paths;

/// 将证书以 armored ASCII 格式保存到本地 keyring。
///
/// 文件名为 `{fingerprint}.asc`，存储在 `~/.hypo/keyring/`。
pub fn save_cert(cert: &Cert) -> Result<(), HypoError> {
    let dir = paths::keyring_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| HypoError::Sequoia(format!("无法创建 keyring 目录: {e}")))?;

    let fingerprint = cert.fingerprint().to_hex();
    let path = dir.join(format!("{fingerprint}.asc"));

    let mut armored = Vec::new();
    cert.armored()
        .serialize(&mut armored)
        .map_err(|e| HypoError::Sequoia(format!("证书序列化失败: {e}")))?;

    std::fs::write(&path, &armored)
        .map_err(|e| HypoError::Sequoia(format!("保存证书失败: {e}")))?;

    Ok(())
}

/// 从本地 keyring 加载证书。
///
/// 按指纹查找 `~/.hypo/keyring/{fingerprint}.asc`。
/// 未找到返回 `None`。
pub fn load_cert(fingerprint: &str) -> Option<Cert> {
    let path = paths::keyring_dir().join(format!("{fingerprint}.asc"));
    let bytes = std::fs::read(&path).ok()?;
    Cert::from_bytes(&bytes).ok()
}

/// 列出本地 keyring 中所有证书的指纹。
pub fn list_certs() -> Vec<String> {
    let dir = paths::keyring_dir();
    let mut fingerprints = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "asc") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    fingerprints.push(stem.to_string());
                }
            }
        }
    }
    fingerprints
}

#[cfg(test)]
mod tests {
    use super::*;
    use sequoia_openpgp::cert::CertBuilder;

    #[test]
    fn test_save_load_list() {
        let (cert, _) = CertBuilder::new()
            .add_userid("test-keyring@example.com")
            .generate()
            .expect("生成测试证书失败");

        let fp = cert.fingerprint().to_hex();

        // 保存
        save_cert(&cert).expect("保存证书失败");

        // 加载
        let loaded = load_cert(&fp).expect("加载证书失败");
        assert_eq!(loaded.fingerprint().to_hex(), fp);

        // 列表
        let list = list_certs();
        assert!(list.contains(&fp));
    }
}
