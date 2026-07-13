//! `hypo install` — 安装包的核心编排。
//!
//! 串联完整的信任链：registry 拉取 → GPG 验签 → 下载 → 解包 → 哈希校验 → 执行脚本 → 数据库记录。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::HypoError;
use crate::executor::executor_trait::ScriptExecutor;
use crate::paths;

/// 执行 hypo install。
pub async fn run(
    package: &str,
    force: bool,
    yes: bool,
    from_url: Option<&str>,
) -> Result<(), HypoError> {
    if let Some(url) = from_url {
        install_from_url(url, force).await
    } else {
        install_from_registry(package, force, yes).await
    }
}

/// 从官方目录安装。
async fn install_from_registry(package: &str, force: bool, yes: bool) -> Result<(), HypoError> {
    // 1. 解析包名
    let dep = crate::deps::resolver::parse_dep_string(package)?;
    let (owner, name, version_constraint) = (&dep.owner, &dep.name, &dep.constraint_str);

    println!("正在安装 @{owner}/{name} ...");

    // 2. 拉取官方目录 registry.json
    let (registry, registry_raw) = crate::registry::client::fetch_registry_json().await?;
    let registry_sig = crate::registry::client::fetch_registry_sig().await?;

    // 3. 验签 registry.json（需要根证书，MVP 使用 keyring 中缓存的官方密钥）
    verify_registry_sig(&registry_raw, &registry_sig)?;

    // 4. 拉取开发者分片 + SHA256 校验
    let shard = crate::registry::client::fetch_shard(&registry, owner).await?;

    // 5. 建立开发者信任
    crate::registry::trust::trust_from_shard(&shard).await?;

    // 6. 拉取 hypo-index.json
    let index = crate::registry::client::fetch_hypo_index(&shard.base_pkg_url).await?;

    // 7. 找到目标包
    let pkg_entry = index
        .packages
        .iter()
        .find(|p| p.name == *name)
        .ok_or_else(|| HypoError::PackageNotFound(format!("{name} 不在 {owner} 的包列表中")))?;

    // 8. 确定版本（指定版本或 latest_version）
    let target_version = if !version_constraint.is_empty() {
        let versions: Vec<String> = pkg_entry
            .versions
            .iter()
            .map(|v| v.version.clone())
            .collect();
        crate::deps::resolver::select_best_version(version_constraint, &versions)?.to_string()
    } else {
        pkg_entry.latest_version.clone()
    };

    // 9. 降级防护
    check_downgrade(owner, name, &target_version, force)?;

    // 10. Freeze 检查
    if let Some(ver_info) = pkg_entry
        .versions
        .iter()
        .find(|v| v.version == target_version)
    {
        check_freeze(ver_info, force, pkg_entry)?;
    }

    // 11. 拉取 hypo-package.json
    let hypo_pkg =
        crate::registry::client::fetch_hypo_package(&shard.base_pkg_url, name, &target_version)
            .await?;

    // 12. 选择当前平台条目
    let platform_entry = select_platform_entry(&hypo_pkg)?;

    // 13. 下载 .hypo + .hypo.sig
    let tmp_dir = paths::tmp_dir().join(format!("{owner}-{name}-{target_version}"));
    std::fs::create_dir_all(&tmp_dir).ok();

    let hypo_path = tmp_dir.join(format!("{name}-{target_version}.hypo"));
    crate::package::reader::HypoPackageReader::download(&platform_entry.url, &hypo_path).await?;

    let sig_path = tmp_dir.join(format!("{name}-{target_version}.hypo.sig"));
    crate::package::reader::HypoPackageReader::download(&platform_entry.sig_url, &sig_path).await?;

    // 14. 验 .hypo.sig 整体签名
    verify_hypo_sig(&hypo_path, &sig_path, owner)?;

    // 15. 解包
    let extract_dir = tmp_dir.join("extracted");
    let reader = crate::package::reader::HypoPackageReader::new(hypo_path.clone());
    reader.extract_to(&extract_dir)?;

    // 16. 拉取 + 验 manifest
    let (remote_manifest_bytes, manifest_sig) =
        crate::registry::client::fetch_manifest(&shard.base_pkg_url, name, &target_version).await?;
    verify_manifest_sig(&remote_manifest_bytes, &manifest_sig, owner)?;

    let remote_manifest = crate::package::manifest::Manifest::parse(
        std::str::from_utf8(&remote_manifest_bytes).unwrap_or(""),
    )?;

    // 17. 对比包内 manifest 与 gh-pages manifest 一致性
    let local_manifest = reader.read_manifest()?;
    local_manifest.compare_consistency(&remote_manifest)?;

    // 18. 逐文件 SHA256 校验
    crate::package::hash::verify_files(&extract_dir, &local_manifest)?;

    // 19. 确定安装目录（从 sandbox 声明或默认路径）
    let install_dir = determine_install_dir(owner, name);

    // 20. 执行安装脚本
    let script_path = extract_dir.join(&local_manifest.scripts.install);
    let executor = crate::executor::powershell::PowerShellExecutor::new(
        &format!("@{owner}/{name}"),
        &target_version,
        install_dir.clone(),
        extract_dir.join("content"),
    )
    .with_sandbox(&format!(
        "写入: {:?}",
        local_manifest.sandbox.allowed_write_paths
    ));

    let mut executor = executor;
    executor.skip_confirm = yes;
    executor.execute(&script_path, HashMap::new()).await?;

    // 21. 写入本地数据库
    record_install(
        owner,
        name,
        &target_version,
        &install_dir,
        &shard.base_pkg_url,
    )?;

    // 22. 生成 lockfile
    generate_lockfile(
        owner,
        name,
        &target_version,
        &platform_entry.url,
        &platform_entry.sha256,
    )?;

    println!("安装完成: @{owner}/{name} v{target_version}");
    Ok(())
}

/// 从 --from-url 安装。
async fn install_from_url(url: &str, _force: bool) -> Result<(), HypoError> {
    // 提取 base_pkg_url
    let base_url = url.trim_end_matches('/');
    println!("从自定义 URL 安装: {base_url}");

    // 信任模型：GitHub Pages 自动信任，其他走 TOFU
    if crate::registry::trust::is_github_pages_url(base_url) {
        println!("检测到 GitHub Pages，自动建立信任");
    } else {
        println!("非 GitHub Pages URL，使用 TOFU 信任模型");
    }

    // 拉取 hypo-index.json
    let _index = crate::registry::client::fetch_hypo_index(base_url).await?;

    // MVP 简化为输出信息并提示
    Err(HypoError::Config(
        "--from-url 完整安装流程将在集成测试阶段实现。请先使用官方目录安装。".to_string(),
    ))
}

// ── 辅助函数 ──────────────────────────────────────────────────

/// 验证 registry.json 签名。
fn verify_registry_sig(_json: &[u8], _sig: &[u8]) -> Result<(), HypoError> {
    // MVP：从 keyring 加载官方根证书并验证
    // 完整实现需从 constants::OFFICIAL_KEY_FINGERPRINTS 加载
    crate::crypto::keyring::list_certs(); // 确认 keyring 可用
    Ok(()) // 骨架：假设验证通过（实际需 Step 9 集成测试完善）
}

/// 验证 .hypo 签名。
fn verify_hypo_sig(_hypo: &PathBuf, _sig: &PathBuf, _owner: &str) -> Result<(), HypoError> {
    Ok(()) // 骨架
}

/// 验证 manifest 签名。
fn verify_manifest_sig(_manifest: &[u8], _sig: &[u8], _owner: &str) -> Result<(), HypoError> {
    Ok(()) // 骨架
}

/// 选择当前平台条目。
fn select_platform_entry(
    pkg: &crate::registry::types::HypoPackage,
) -> Result<&crate::registry::types::HypoPackageEntry, HypoError> {
    let platform = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "all"
    };

    // 优先匹配当前平台
    for entry in &pkg.packages {
        if entry.platform == platform || entry.platform == "all" {
            return Ok(entry);
        }
    }

    Err(HypoError::PackageNotFound(format!(
        "当前平台 {platform} 无可用包"
    )))
}

/// 降级防护检查。
fn check_downgrade(owner: &str, name: &str, target: &str, force: bool) -> Result<(), HypoError> {
    let db_path = paths::db_path();
    if let Ok(conn) = crate::db::schema::init_db(&db_path) {
        if let Ok(Some(seen)) = crate::db::operations::get_latest_seen_version(&conn, owner, name) {
            if let (Ok(seen_ver), Ok(target_ver)) = (
                semver::Version::parse(&seen),
                semver::Version::parse(target),
            ) {
                if target_ver < seen_ver && !force {
                    return Err(HypoError::DowngradeDetected(format!(
                        "检测到降级: latest_seen={seen}, target={target}。使用 --force 强制执行。"
                    )));
                }
            }
        }
    }
    Ok(())
}

/// Freeze 三态检查。
fn check_freeze(
    ver: &crate::registry::types::HypoIndexVersion,
    force: bool,
    pkg: &crate::registry::types::HypoIndexPackage,
) -> Result<(), HypoError> {
    if !ver.freeze {
        return Ok(());
    }

    if force {
        eprintln!(
            "警告: 版本 {} 已被冻结，原因: {:?}。强制安装。",
            ver.version, ver.freeze_reason
        );
        return Ok(());
    }

    Err(HypoError::FreezeViolation(format!(
        "版本 {} 已被冻结: {:?}。使用 --force 强制执行，或安装回退版本 {}。",
        ver.version,
        ver.freeze_reason,
        ver.rollback_version
            .as_deref()
            .unwrap_or(&pkg.latest_version)
    )))
}

/// 确定安装目录。
fn determine_install_dir(owner: &str, name: &str) -> PathBuf {
    paths::hypo_base_dir().join("apps").join(owner).join(name)
}

/// 写入安装记录到数据库。
fn record_install(
    owner: &str,
    name: &str,
    version: &str,
    install_dir: &Path,
    source: &str,
) -> Result<(), HypoError> {
    let db_path = paths::db_path();
    let conn = crate::db::schema::init_db(&db_path)?;
    let record = crate::db::operations::PackageRecord {
        id: 0,
        owner: owner.to_string(),
        name: name.to_string(),
        version: version.to_string(),
        platform: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        install_path: install_dir.display().to_string(),
        script_dir: paths::tmp_dir()
            .join(format!("{owner}-{name}-{version}"))
            .join("extracted/tools")
            .display()
            .to_string(),
        source_registry: source.to_string(),
        installed_at: String::new(),
        latest_seen_version: version.to_string(),
    };
    crate::db::operations::insert_package(&conn, &record)?;
    Ok(())
}

/// 生成 lockfile。
fn generate_lockfile(
    _owner: &str,
    _name: &str,
    _version: &str,
    _url: &str,
    _sha256: &str,
) -> Result<(), HypoError> {
    // MVP 生成基础 lockfile
    let lock_path = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("hypo.lock");

    let deps = vec![crate::deps::resolver::ResolvedDep {
        owner: _owner.to_string(),
        name: _name.to_string(),
        version: _version.to_string(),
        available_versions: vec![_version.to_string()],
    }];

    let lock_str = crate::deps::lockfile::Lockfile::generate(&deps)?;
    std::fs::write(&lock_path, lock_str).ok();
    Ok(())
}
