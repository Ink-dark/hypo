/// 烟雾测试数据生成器。
/// 运行: cargo run --bin gen-smoke-data
/// 生成: tools/smoke-test/registry/ 下的完整 registry 结构 + .hypo 包 + GPG 签名

use std::io::Write;
use std::path::PathBuf;

use sequoia_openpgp::cert::prelude::*;
use sequoia_openpgp::policy::StandardPolicy;
use sequoia_openpgp::serialize::stream::{Message, Signer};
use sequoia_openpgp::serialize::Serialize as _;
use sha2::{Digest, Sha256};

const OUT: &str = "tools/smoke-test";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(OUT);
    std::fs::create_dir_all(out.join("registry/a"))?;
    std::fs::create_dir_all(out.join("registry/test-owner"))?;

    // ── 1. 生成 GPG 密钥 ──────────────────────────────────────────
    println!("[1/6] 生成测试 GPG 密钥...");
    let (cert, _rev) = CertBuilder::new()
        .add_userid("smoke-test@hypo.local")
        .add_signing_subkey()
        .generate()?;

    let fp = cert.fingerprint().to_hex();
    let cert_bytes = {
        let mut buf = Vec::new();
        cert.armored().serialize(&mut buf)?;
        buf
    };
    std::fs::write(out.join("test-key.asc"), &cert_bytes)?;
    std::fs::write(out.join("test-key-fingerprint.txt"), &fp)?;
    println!("   指纹: {fp}");

    // ── 2. 创建 .hypo 包 ──────────────────────────────────────────
    println!("[2/6] 创建测试 .hypo 包...");

    let install_ps1 = r#"
Write-Host "=== hypo 烟雾测试：安装成功！ ==="
$install_dir = $env:HYPO_INSTALL_DIR
Write-Host "安装目录: $install_dir"
New-Item -ItemType Directory -Force -Path $install_dir | Out-Null
Set-Content -Path "$install_dir\installed.txt" -Value "hypo smoke test - installed at $(Get-Date)"
Write-Host "已创建标记文件: $install_dir\installed.txt"
"#;

    let manifest_content = format!(
        r#"[package]
name = "smoke-test"
version = "0.1.0"
description = "hypo smoke test package"
author = "test-owner"
repo = "test-owner/smoke-test"
platform = "windows"
arch = ["x86_64"]

[scripts]
install = "tools/install.ps1"

[interpreter]
type = "powershell"

[sandbox]
allowed_write_paths = ["$env:LOCALAPPDATA/smoke-test", "$env:USERPROFILE/.hypo/apps"]
allowed_network_egress = []

[dependencies]
hypo = []
system = []

[hashes]
"tools/install.ps1" = "{}"
"#, format!("{:x}", Sha256::digest(install_ps1.as_bytes()))
    );

    // 创建 ZIP 包
    let hypo_path = out.join("packages/smoke-test-0.1.0-windows.hypo");
    std::fs::create_dir_all(hypo_path.parent().unwrap())?;

    let file = std::fs::File::create(&hypo_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("manifest.toml", options)?;
    zip.write_all(manifest_content.as_bytes())?;
    zip.start_file("tools/install.ps1", options)?;
    zip.write_all(install_ps1.as_bytes())?;
    zip.finish()?;
    println!("   包: {}", hypo_path.display());

    // ── 3. 签名 .hypo 包 ──────────────────────────────────────────
    println!("[3/6] 签名 .hypo 包...");
    let hypo_data = std::fs::read(&hypo_path)?;
    let sig = sign_detached(&cert, &hypo_data)?;
    let sig_path = out.join("packages/smoke-test-0.1.0-windows.hypo.sig");
    std::fs::write(&sig_path, &sig)?;

    // ── 4. 计算 SHA256 ────────────────────────────────────────────
    println!("[4/6] 计算哈希...");
    let hypo_sha256 = format!("{:x}", Sha256::digest(&hypo_data));
    let _sig_sha256 = format!("{:x}", Sha256::digest(&sig));

    // ── 5. 创建 registry JSON 文件 ────────────────────────────────
    println!("[5/6] 创建 registry JSON 文件...");

    // 先写分片，计算哈希
    let shard_json = serde_json::json!({
        "github_username": "test-owner",
        "gpg_key_fingerprints": [fp],
        "base_pkg_url": "http://localhost:8765",
        "registered_at": "2026-01-01T00:00:00Z"
    });
    let shard_bytes = serde_json::to_vec(&shard_json)?;
    std::fs::write(out.join("registry/t/test-owner.json"), &shard_bytes)?;
    let shard_hash = format!("sha256:{}", format!("{:x}", Sha256::digest(&shard_bytes)));

    // registry.json（含实际分片哈希）
    let registry_json = serde_json::json!({
        "schema_version": 1,
        "snapshot_version": 1,
        "shards": ["t"],
        "shard_hashes": {
            "t/test-owner.json": shard_hash
        },
        "official_key_fingerprints": [fp],
        "key_rotation": {
            "old_key_fingerprint": "",
            "old_key_retired_at": "2099-01-01T00:00:00Z",
            "transition_period_days": 90
        },
        "key_update_url": "http://localhost:8765/keys/current.json",
        "mirrors": []
    });
    std::fs::write(
        out.join("registry/registry.json"),
        serde_json::to_string_pretty(&registry_json)?,
    )?;

    // registry.sig（用测试密钥签名）
    let reg_bytes = serde_json::to_vec(&registry_json)?;
    let reg_sig = sign_detached(&cert, &reg_bytes)?;
    std::fs::write(out.join("registry/registry.sig"), &reg_sig)?;

    // hypo-index.json (放在 registry 根，模拟 base_pkg_url)
    let hypo_index = serde_json::json!({
        "schema_version": 1,
        "owner": "test-owner",
        "packages": [{
            "name": "smoke-test",
            "description": "Smoke test package",
            "repo": "test-owner/smoke-test",
            "latest_version": "0.1.0",
            "versions": [{
                "version": "0.1.0",
                "released_at": "2026-07-01T00:00:00Z",
                "package_index_path": "smoke-test/0.1.0/hypo-package.json",
                "manifest_path": "smoke-test/0.1.0/manifest.toml",
                "manifest_sig_path": "smoke-test/0.1.0/manifest.toml.sig",
                "freeze": false,
                "freeze_reason": null,
                "rollback_version": null,
                "hypo_deps": [],
                "system_deps": []
            }]
        }]
    });
    std::fs::write(
        // 写在 HTTP 根目录（base_pkg_url=/）
        out.join("hypo-index.json"),
        serde_json::to_string_pretty(&hypo_index)?,
    )?;

    // hypo-package.json
    let hypo_pkg = serde_json::json!({
        "schema_version": 1,
        "name": "smoke-test",
        "version": "0.1.0",
        "packages": [{
            "platform": "windows",
            "arch": ["x86_64"],
            "url": format!("http://localhost:8765/packages/smoke-test-0.1.0-windows.hypo"),
            "size": hypo_data.len(),
            "sha256": hypo_sha256,
            "sig_url": format!("http://localhost:8765/packages/smoke-test-0.1.0-windows.hypo.sig")
        }]
    });
    let pkg_dir = out.join("smoke-test/0.1.0");
    std::fs::create_dir_all(&pkg_dir)?;
    std::fs::write(
        pkg_dir.join("hypo-package.json"),
        serde_json::to_string_pretty(&hypo_pkg)?,
    )?;

    // manifest.toml + manifest.toml.sig
    std::fs::write(pkg_dir.join("manifest.toml"), &manifest_content)?;
    let manifest_sig = sign_detached(&cert, manifest_content.as_bytes())?;
    std::fs::write(pkg_dir.join("manifest.toml.sig"), &manifest_sig)?;

    // ── 6. 保存 GPG 公钥到 keyring ───────────────────────────────
    println!("[6/6] 缓存公钥到 keyring...");
    let keyring_dir = out.join("keyring");
    std::fs::create_dir_all(&keyring_dir)?;
    std::fs::write(keyring_dir.join(format!("{fp}.asc")), &cert_bytes)?;

    println!("\n✅ 烟雾测试数据生成完毕！");
    println!("   输出目录: {}", out.display());
    println!("   GPG 指纹: {fp}");
    println!("\n启动测试:");
    println!("   cd tools/smoke-test && python -m http.server 8765");
    println!("   set HYPO_REGISTRY_URL=http://localhost:8765/registry");
    println!("   copy tools\\smoke-test\\keyring\\* %USERPROFILE%\\.hypo\\keyring\\");
    println!("   cargo run -- install @test-owner/smoke-test");

    Ok(())
}

fn sign_detached(cert: &sequoia_openpgp::Cert, data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let p = &StandardPolicy::new();
    let keypair = cert
        .keys()
        .secret()
        .with_policy(p, None)
        .supported()
        .alive()
        .revoked(false)
        .for_signing()
        .next()
        .ok_or("no signing key")?
        .key()
        .clone()
        .into_keypair()?;

    let mut sig = Vec::new();
    {
        let message = Message::new(&mut sig);
        let mut message = Signer::new(message, keypair)?.detached().build()?;
        message.write_all(data)?;
        message.finalize()?;
    }
    Ok(sig)
}
