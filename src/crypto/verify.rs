//! GPG 分离签名验证。
//!
//! 使用 `sequoia-openpgp` 纯 Rust 实现，通过 [`VerificationHelper`] trait
//! 向验证器提供证书（公钥）。

use sequoia_openpgp::parse::stream::{
    DetachedVerifierBuilder, MessageStructure, VerificationHelper,
};
use sequoia_openpgp::parse::Parse;
use sequoia_openpgp::policy::StandardPolicy;
use sequoia_openpgp::Cert;

use crate::error::HypoError;
use crate::registry::types::KeyRotation;

// ── VerificationHelper 实现 ──────────────────────────────────────

/// 简单的证书提供者：持有单个证书，按需返回。
struct SingleCertHelper {
    cert: Cert,
}

impl VerificationHelper for SingleCertHelper {
    fn get_certs(
        &mut self,
        _ids: &[sequoia_openpgp::KeyHandle],
    ) -> sequoia_openpgp::Result<Vec<Cert>> {
        Ok(vec![self.cert.clone()])
    }

    fn check(&mut self, structure: MessageStructure) -> sequoia_openpgp::Result<()> {
        // 遍历验证结果，任一失败则传播错误
        for (i, layer) in structure.iter().enumerate() {
            if let sequoia_openpgp::parse::stream::MessageLayer::SignatureGroup { results } = layer
            {
                for result in results {
                    if let Err(ref e) = result {
                        return Err(anyhow::anyhow!("第 {i} 层签名验证失败: {e}"));
                    }
                }
            }
        }
        Ok(())
    }
}

// ── 底层验证原语 ─────────────────────────────────────────────────

/// 用指定证书验证数据的分离签名。
///
/// 这是底层验证原语，不检查指纹白名单。
/// 指纹检查由上层函数（[`verify_registry_sig`]、[`verify_hypo_sig`] 等）负责。
pub fn verify_signature(data: &[u8], sig: &[u8], cert: &Cert) -> Result<(), HypoError> {
    let policy = StandardPolicy::new();
    let helper = SingleCertHelper { cert: cert.clone() };

    let mut verifier = DetachedVerifierBuilder::from_bytes(sig)
        .map_err(|e| HypoError::SignatureVerification(format!("签名解析失败: {e}")))?
        .with_policy(&policy, None, helper)
        .map_err(|e| HypoError::SignatureVerification(format!("验证器构建失败: {e}")))?;

    verifier
        .verify_bytes(data)
        .map_err(|e| HypoError::SignatureVerification(format!("签名验证失败: {e}")))?;

    Ok(())
}

// ── 上层验证函数 ─────────────────────────────────────────────────

/// 验证官方目录 `registry.sig` 对 `registry.json` 的签名。
///
/// 需要提供受信任的根证书。
/// 调用方需先通过 [`check_fingerprint_trusted`] 确认 cert 的指纹
/// 属于硬编码的信任集合。
pub fn verify_registry_sig(
    registry_json: &[u8],
    registry_sig: &[u8],
    cert: &Cert,
) -> Result<(), HypoError> {
    verify_signature(registry_json, registry_sig, cert)
}

/// 验证 `.hypo.sig` 对 `.hypo` 包文件的开发者签名。
pub fn verify_hypo_sig(hypo_data: &[u8], sig_data: &[u8], cert: &Cert) -> Result<(), HypoError> {
    verify_signature(hypo_data, sig_data, cert)
}

/// 验证 `manifest.toml.sig` 对 `manifest.toml` 的开发者签名。
pub fn verify_manifest_sig(manifest: &[u8], sig: &[u8], cert: &Cert) -> Result<(), HypoError> {
    verify_signature(manifest, sig, cert)
}

// ── 指纹信任检查 ─────────────────────────────────────────────────

/// 检查签名者的指纹是否在受信任列表中。
///
/// 返回 `Ok(())` 若指纹匹配任一受信任条目，否则返回 `SignatureVerification` 错误。
pub fn check_fingerprint_trusted(
    cert: &Cert,
    trusted_fingerprints: &[String],
) -> Result<(), HypoError> {
    let fp = cert.fingerprint().to_hex();
    if trusted_fingerprints
        .iter()
        .any(|t| t.eq_ignore_ascii_case(&fp))
    {
        Ok(())
    } else {
        Err(HypoError::SignatureVerification(format!(
            "证书指纹 {fp} 不在受信任列表中"
        )))
    }
}

// ── 公钥轮换过渡期 ───────────────────────────────────────────────

/// 判断旧密钥轮换过渡期是否已过期。
///
/// 对比 `key_rotation.old_key_retired_at` 与当前 UTC 时间：
/// - 返回 `true` 表示旧密钥已退役，应拒绝 `registry.sig.old`
/// - 返回 `false` 表示仍在过渡期内，旧密钥签名仍可接受
pub fn is_old_key_retired(rotation: &KeyRotation) -> bool {
    parse_iso8601(&rotation.old_key_retired_at).is_none_or(|retired_at| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now >= retired_at
    })
}

/// 简单的 ISO 8601 时间戳解析（仅支持 `YYYY-MM-DDTHH:MM:SSZ` 格式）。
///
/// 返回 Unix 时间戳（秒）。
fn parse_iso8601(s: &str) -> Option<u64> {
    let s = s.strip_suffix('Z')?;
    let parts: Vec<&str> = s.split('T').collect();
    if parts.len() != 2 {
        return None;
    }
    let date_parts: Vec<&str> = parts[0].split('-').collect();
    let time_parts: Vec<&str> = parts[1].split(':').collect();
    if date_parts.len() != 3 || time_parts.len() != 3 {
        return None;
    }

    let year: i32 = date_parts[0].parse().ok()?;
    let month: u32 = date_parts[1].parse().ok()?;
    let day: u32 = date_parts[2].parse().ok()?;
    let hour: u32 = time_parts[0].parse().ok()?;
    let min: u32 = time_parts[1].parse().ok()?;
    let sec: u32 = time_parts[2].parse().ok()?;

    let days = days_from_epoch(year, month, day);
    Some(days * 86400 + hour as u64 * 3600 + min as u64 * 60 + sec as u64)
}

/// 简化的纪元天数计算（1970-01-01 为第 0 天）。
fn days_from_epoch(year: i32, month: u32, day: u32) -> u64 {
    let y = year as u64;
    let m = month as u64;
    let d = day as u64;

    let y_days = (y - 1970) * 365 + (y - 1969) / 4;
    let month_days = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let leap = if is_leap(year) && m > 2 { 1 } else { 0 };

    y_days + month_days[(m - 1) as usize] + d - 1 + leap
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

// ── 单元测试 ────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use sequoia_openpgp::cert::CertBuilder;
    use sequoia_openpgp::policy::StandardPolicy;
    use sequoia_openpgp::serialize::stream::{Message, Signer};

    /// 生成一个测试用证书（包含签名子密钥）。
    fn generate_test_cert() -> Cert {
        let (cert, _revocation) = CertBuilder::new()
            .add_userid("test@hypo.example")
            .add_signing_subkey()
            .generate()
            .expect("生成测试证书失败");
        cert
    }

    /// 对数据生成分离签名。
    fn sign_data_detached(cert: &Cert, data: &[u8]) -> Vec<u8> {
        let p = &StandardPolicy::new();

        let signing_keypair = cert
            .keys()
            .secret()
            .with_policy(p, None)
            .supported()
            .alive()
            .revoked(false)
            .for_signing()
            .next()
            .expect("未找到签名密钥")
            .key()
            .clone()
            .into_keypair()
            .expect("提取密钥对失败");

        let mut sink = Vec::new();
        {
            let message = Message::new(&mut sink);
            let mut message = Signer::new(message, signing_keypair)
                .detached()
                .build()
                .expect("创建 Signer 失败");
            use std::io::Write;
            message.write_all(data).expect("写入签名数据失败");
            message.finalize().expect("完成签名失败");
        }
        sink
    }

    #[test]
    fn test_verify_valid_signature() {
        let cert = generate_test_cert();
        let data = b"hello hypo";
        let sig = sign_data_detached(&cert, data);

        verify_signature(data, &sig, &cert).expect("有效签名验证应通过");
    }

    #[test]
    fn test_verify_tampered_data_rejected() {
        let cert = generate_test_cert();
        let data = b"original data";
        let sig = sign_data_detached(&cert, data);

        let result = verify_signature(b"tampered data", &sig, &cert);
        assert!(result.is_err(), "篡改数据应被拒绝");
    }

    #[test]
    fn test_verify_wrong_cert_rejected() {
        let cert_a = generate_test_cert();
        let cert_b = generate_test_cert();
        let data = b"test data";
        let sig = sign_data_detached(&cert_a, data);

        let result = verify_signature(data, &sig, &cert_b);
        assert!(result.is_err(), "错误证书应被拒绝");
    }

    #[test]
    fn test_check_fingerprint_trusted() {
        let cert = generate_test_cert();
        let fp = cert.fingerprint().to_hex();
        assert!(check_fingerprint_trusted(&cert, std::slice::from_ref(&fp)).is_ok());
        assert!(check_fingerprint_trusted(&cert, &["DEADBEEF".into()]).is_err());
    }

    #[test]
    fn test_old_key_not_retired_in_future() {
        let rotation = KeyRotation {
            old_key_fingerprint: "F6E5D4".into(),
            old_key_retired_at: "2099-01-01T00:00:00Z".into(),
            transition_period_days: 90,
        };
        assert!(!is_old_key_retired(&rotation));
    }

    #[test]
    fn test_old_key_retired_in_past() {
        let rotation = KeyRotation {
            old_key_fingerprint: "F6E5D4".into(),
            old_key_retired_at: "2020-01-01T00:00:00Z".into(),
            transition_period_days: 90,
        };
        assert!(is_old_key_retired(&rotation));
    }
}
