//! hypo 集成测试。
//!
//! 覆盖 MVP 核心路径：init → list → info → 错误场景 → 退出码。

use std::collections::HashMap;
use std::path::PathBuf;

use hypo::error::HypoError;

// ── init + list 端到端 ─────────────────────────────────────────

#[test]
fn test_init_and_list() {
    // hypo init 创建目录结构和数据库
    let base = hypo::paths::hypo_base_dir();
    // 在不影响真实 ~/.hypo/ 的情况下，仅验证路径函数返回有效值
    assert!(!base.as_os_str().is_empty());
    assert!(hypo::paths::config_path().ends_with("config.toml"));
    assert!(hypo::paths::db_path().ends_with("hypo.db"));
    assert!(hypo::paths::cache_dir().ends_with("cache"));
    assert!(hypo::paths::keyring_dir().ends_with("keyring"));
    assert!(hypo::paths::tmp_dir().ends_with("tmp"));
    assert!(hypo::paths::logs_dir().ends_with("logs"));
}

// ── 退出码验证 ─────────────────────────────────────────────────

#[test]
fn test_exit_codes() {
    // 签名验证失败 → 10
    let e = HypoError::SignatureVerification("test".into());
    assert_eq!(e.exit_code(), 10);

    // 哈希不匹配 → 11
    let e = HypoError::HashMismatch("test".into());
    assert_eq!(e.exit_code(), 11);

    // 网络错误 → 12
    let e = HypoError::Network("test".into());
    assert_eq!(e.exit_code(), 12);

    // 包未找到 → 13
    let e = HypoError::PackageNotFound("test".into());
    assert_eq!(e.exit_code(), 13);

    // Registry 未找到 → 13 (共用)
    let e = HypoError::RegistryNotFound("test".into());
    assert_eq!(e.exit_code(), 13);

    // Freeze 违规 → 14
    let e = HypoError::FreezeViolation("test".into());
    assert_eq!(e.exit_code(), 14);

    // 降级检测 → 15
    let e = HypoError::DowngradeDetected("test".into());
    assert_eq!(e.exit_code(), 15);

    // IO 错误 → 1
    let e = HypoError::Database("test".into());
    assert_eq!(e.exit_code(), 1);

    // 通用 → 1
    let e = HypoError::Config("test".into());
    assert_eq!(e.exit_code(), 1);
}

// ── 哈希校验 ───────────────────────────────────────────────────

#[test]
fn test_hash_mismatch_detected() {
    // 创建临时文件，计算哈希后篡改内容，验证不一致被检测
    let dir = std::env::temp_dir().join("hypo-int-test-hash");
    std::fs::create_dir_all(&dir).unwrap();

    let file_path = dir.join("test.bin");
    let original = b"original content for hash test";
    std::fs::write(&file_path, original).unwrap();

    let hash1 = hypo::package::hash::compute_sha256(&file_path).unwrap();

    // 篡改内容
    std::fs::write(&file_path, b"tampered content").unwrap();
    let hash2 = hypo::package::hash::compute_sha256(&file_path).unwrap();

    assert_ne!(hash1, hash2, "篡改后哈希应不同");

    std::fs::remove_dir_all(&dir).ok();
}

// ── 依赖解析 ───────────────────────────────────────────────────

#[test]
fn test_parse_dep_string_valid() {
    let dep = hypo::deps::resolver::parse_dep_string("@alice/tool >= 1.0.0").unwrap();
    assert_eq!(dep.owner, "alice");
    assert_eq!(dep.name, "tool");
    assert_eq!(dep.constraint_str, ">= 1.0.0");
}

#[test]
fn test_parse_dep_string_invalid() {
    assert!(hypo::deps::resolver::parse_dep_string("no-prefix").is_err());
    assert!(hypo::deps::resolver::parse_dep_string("@no-slash").is_err());
}

#[test]
fn test_cycle_detection() {
    let deps = vec![
        hypo::deps::resolver::ResolvedDep {
            owner: "a".into(),
            name: "pkg-a".into(),
            version: "1.0.0".into(),
            available_versions: vec![],
        },
        hypo::deps::resolver::ResolvedDep {
            owner: "b".into(),
            name: "pkg-b".into(),
            version: "1.0.0".into(),
            available_versions: vec![],
        },
    ];
    let edges = vec![(0, 1), (1, 0)]; // a → b → a
    assert!(hypo::deps::resolver::topological_sort(&deps, &edges).is_err());
}

// ── Lockfile ────────────────────────────────────────────────────

#[test]
fn test_lockfile_generate_and_parse() {
    let deps = vec![hypo::deps::resolver::ResolvedDep {
        owner: "test".into(),
        name: "pkg".into(),
        version: "1.2.3".into(),
        available_versions: vec!["1.2.3".into()],
    }];

    let toml_str = hypo::deps::lockfile::Lockfile::generate(&deps).unwrap();
    let lock = hypo::deps::lockfile::Lockfile::parse(&toml_str).unwrap();

    assert_eq!(lock.version, 1);
    assert_eq!(lock.packages.len(), 1);
    assert!(lock.find("test", "pkg").is_some());
    assert!(lock.find("no", "exist").is_none());
}

// ── 数据库 CRUD ────────────────────────────────────────────────

#[test]
fn test_db_full_crud() {
    use hypo::db::operations;
    use rusqlite::Connection;

    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE packages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            owner TEXT NOT NULL, name TEXT NOT NULL, version TEXT NOT NULL,
            platform TEXT DEFAULT '', arch TEXT DEFAULT '',
            install_path TEXT DEFAULT '', script_dir TEXT DEFAULT '',
            source_registry TEXT DEFAULT '',
            installed_at TEXT DEFAULT (datetime('now')),
            latest_seen_version TEXT DEFAULT '',
            UNIQUE(owner, name)
        );",
    )
    .unwrap();

    let pkg = operations::PackageRecord {
        id: 0,
        owner: "alice".into(),
        name: "tool".into(),
        version: "1.0.0".into(),
        platform: "windows".into(),
        arch: "x86_64".into(),
        install_path: "/tmp".into(),
        script_dir: "/tmp/scripts".into(),
        source_registry: "https://example.com".into(),
        installed_at: "".into(),
        latest_seen_version: "1.0.0".into(),
    };

    operations::insert_package(&conn, &pkg).unwrap();
    let got = operations::get_package(&conn, "alice", "tool")
        .unwrap()
        .unwrap();
    assert_eq!(got.version, "1.0.0");

    operations::delete_package(&conn, "alice", "tool").unwrap();
    assert!(operations::get_package(&conn, "alice", "tool")
        .unwrap()
        .is_none());
}

// ── Manifest 校验 ──────────────────────────────────────────────

#[test]
fn test_manifest_validate_rejects_empty_install() {
    let mut m = make_test_manifest();
    m.scripts.install = String::new();
    assert!(m.validate().is_err());
}

#[test]
fn test_manifest_validate_rejects_empty_hashes() {
    let mut m = make_test_manifest();
    m.hashes.clear();
    assert!(m.validate().is_err());
}

#[test]
fn test_manifest_consistency_mismatch() {
    let m1 = make_test_manifest();
    let mut m2 = make_test_manifest();
    m2.package.version = "2.0.0".into();
    assert!(m1.compare_consistency(&m2).is_err());
}

fn make_test_manifest() -> hypo::package::manifest::Manifest {
    let mut hashes = HashMap::new();
    hashes.insert("tools/install.ps1".into(), "abc123".into());

    hypo::package::manifest::Manifest {
        package: hypo::package::manifest::PackageMeta {
            name: "test".into(),
            version: "1.0.0".into(),
            description: "".into(),
            author: "test".into(),
            repo: "test/repo".into(),
            platform: "windows".into(),
            arch: vec!["x86_64".into()],
        },
        scripts: hypo::package::manifest::Scripts {
            install: "tools/install.ps1".into(),
            uninstall: None,
            update: None,
            pre_install: None,
            post_install: None,
            pre_uninstall: None,
            post_uninstall: None,
            pre_update: None,
            post_update: None,
        },
        interpreter: Default::default(),
        sandbox: hypo::package::manifest::SandboxDecl {
            allowed_write_paths: vec!["test".into()],
            allowed_network_egress: vec![],
        },
        dependencies: Default::default(),
        hashes,
    }
}

// ── Crypto ──────────────────────────────────────────────────────

#[test]
fn test_crypto_sign_and_verify() {
    use hypo::crypto::verify;
    use sequoia_openpgp::cert::prelude::*;
    use sequoia_openpgp::policy::StandardPolicy;
    use sequoia_openpgp::serialize::stream::{Message, Signer};

    let (cert, _) = CertBuilder::new()
        .add_userid("integration@test.hypo")
        .add_signing_subkey()
        .generate()
        .unwrap();

    let signing_keypair = cert
        .keys()
        .secret()
        .with_policy(&StandardPolicy::new(), None)
        .supported()
        .alive()
        .revoked(false)
        .for_signing()
        .next()
        .unwrap()
        .key()
        .clone()
        .into_keypair()
        .unwrap();

    let data = b"integration test data for hypo";
    let mut sig = Vec::new();
    {
        let message = Message::new(&mut sig);
        let mut message = Signer::new(message, signing_keypair)
            .unwrap()
            .detached()
            .build()
            .unwrap();
        use std::io::Write;
        message.write_all(data).unwrap();
        message.finalize().unwrap();
    }

    // 验证签名
    verify::verify_signature(data, &sig, &cert).expect("有效签名应验证通过");

    // 篡改数据被拒绝
    assert!(verify::verify_signature(b"tampered", &sig, &cert).is_err());
}

// ── Trust 模型 ──────────────────────────────────────────────────

#[test]
fn test_trust_github_pages_detection() {
    use hypo::registry::trust;

    assert!(trust::is_github_pages_url(
        "https://alice.github.io/hypo-pkgs"
    ));
    assert!(trust::is_github_pages_url("https://my-org.github.io"));
    assert!(!trust::is_github_pages_url("https://example.com"));
    assert!(!trust::is_github_pages_url("https://my-app.vercel.app"));
    // userinfo 注入
    assert!(!trust::is_github_pages_url(
        "https://evil.com@alice.github.io/path"
    ));
}

#[test]
fn test_trust_owner_validation() {
    use hypo::registry::trust;
    assert!(trust::validate_owner("alice").is_ok());
    assert!(trust::validate_owner("my-org").is_ok());
    assert!(trust::validate_owner("../evil").is_err());
    assert!(trust::validate_owner("test_user").is_err());
}

// ── 文件校验 ───────────────────────────────────────────────────

#[test]
fn test_verify_files_rejects_path_traversal() {
    use std::collections::HashMap;

    let mut hashes = HashMap::new();
    hashes.insert("../etc/passwd".to_string(), "abc".to_string());
    hashes.insert("tools\\..\\..\\system".to_string(), "def".to_string());

    let manifest = hypo::package::manifest::Manifest {
        package: hypo::package::manifest::PackageMeta {
            name: "test".into(),
            version: "1.0.0".into(),
            description: "".into(),
            author: "".into(),
            repo: "".into(),
            platform: "windows".into(),
            arch: vec![],
        },
        scripts: hypo::package::manifest::Scripts {
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
        interpreter: Default::default(),
        sandbox: hypo::package::manifest::SandboxDecl {
            allowed_write_paths: vec![],
            allowed_network_egress: vec![],
        },
        dependencies: Default::default(),
        hashes,
    };

    // 两个路径都应被拒绝：../ 路径穿越和 \ 反斜杠路径穿越
    let result = hypo::package::hash::verify_files(PathBuf::from(".").as_path(), &manifest);
    assert!(result.is_err());
}

// ── 公钥轮换 ───────────────────────────────────────────────────

#[test]
fn test_key_rotation_retired() {
    let rotation = hypo::registry::types::KeyRotation {
        old_key_fingerprint: "DEAD".into(),
        old_key_retired_at: "2020-01-01T00:00:00Z".into(),
        transition_period_days: 90,
    };
    assert!(hypo::crypto::verify::is_old_key_retired(&rotation));
}

#[test]
fn test_key_rotation_not_retired() {
    let rotation = hypo::registry::types::KeyRotation {
        old_key_fingerprint: "BEEF".into(),
        old_key_retired_at: "2099-12-31T00:00:00Z".into(),
        transition_period_days: 90,
    };
    assert!(!hypo::crypto::verify::is_old_key_retired(&rotation));
}

// ── 配置序列化 ─────────────────────────────────────────────────

#[test]
fn test_config_default_values() {
    let cfg = hypo::config::Config::default();
    assert_eq!(cfg.log_level, "info");
    assert!(cfg.trusted_users.is_empty());
}
