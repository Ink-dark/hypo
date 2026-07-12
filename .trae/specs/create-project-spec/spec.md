# Create Project Specification (docs/SPEC.md) Spec

## Why

hypo 项目目前只有 roadmap.md（开发路线图）和 README.md（一行简介），缺少一份正式的项目规格说明文档。用户要求明确界定：hypo 是**通用软件安装/卸载/更新管理器**（平替 winget/Scoop/Chocolatey），而非仅限 Rust 生态。同时需要确立编码标准（如严禁 unsafe），为后续所有开发提供规范基线。

## What Changes

- 创建 `docs/SPEC.md`：项目规格说明书，涵盖以下章节：
  1. **项目定位**：明确 hypo 为通用包管理器，支持任意语言的软件分发
  2. **编码标准**：严禁 unsafe、Rust edition、错误处理规范、文档要求、CI 标准
  3. **架构规格**：模块结构、信任链、包格式、Registry 结构
  4. **安全规格**：GPG 双签、哈希校验、降级防护、信任模型
  5. **CLI 规格**：子命令集、参数规范、退出码
  6. **兼容性规格**：平台支持、脚本解释器支持、SemVer 合规
  7. **数据结构规格**：registry.json / hypo-index.json / hypo-package.json / manifest.toml 的 schema 定义

## Impact

- Affected specs: 无（首个 spec）
- Affected code: 全项目——docs/SPEC.md 是所有后续开发的规范基线，所有代码必须符合此文档定义的标准

## ADDED Requirements

### Requirement: 通用包管理器定位

hypo SHALL be a general-purpose software package manager that handles installation, uninstallation, and updates of **any** software regardless of programming language or runtime. It is NOT a Rust-only tool. It serves as a decentralized alternative to winget, Scoop, Chocolatey, and similar tools.

#### Scenario: 安装非 Rust 软件
- **WHEN** 开发者发布一个 Python 工具的 .hypo 包（manifest 中 `interpreter.type = "python"`）
- **THEN** hypo 能正常下载、验签、执行安装脚本，完成 Python 工具的安装

#### Scenario: 安装原生二进制软件
- **WHEN** 开发者发布一个 C++ 编译的原生二进制工具的 .hypo 包（install.ps1 负责解压二进制到目标路径）
- **THEN** hypo 能正常下载、验签、执行安装脚本，完成二进制工具的安装

#### Scenario: 跨平台软件安装
- **WHEN** 开发者发布 `platform: "all"` 的 .hypo 包（纯脚本/资源包，由脚本自身处理平台差异）
- **THEN** hypo 在 Windows/Linux/macOS 上都能安装该包

### Requirement: 严禁 unsafe 代码

The project SHALL NOT contain any `unsafe` Rust code in the main codebase. All unsafe operations (FFI, platform-specific low-level API calls) SHALL be delegated to vetted third-party crates from the official dependency list.

#### Scenario: 代码审查发现 unsafe
- **WHEN** 代码审查或 CI 检查发现 `unsafe` 关键字（排除 dependencies 中的第三方 crate）
- **THEN** 构建失败，CI 报错

#### Scenario: 需要调用平台 API
- **WHEN** 需要调用 Windows API（如 ETW、AppContainer）或 Linux API（如 Landlock、seccomp）
- **THEN** 必须通过 `windows` / `nix` / `landlock` 等经过审核的第三方 crate 间接调用，禁止在 hypo 源码中直接使用 unsafe FFI

### Requirement: Rust 编码标准

The project SHALL adhere to the following Rust coding standards:
- Rust edition 2021, stable toolchain
- `clippy` with `-D warnings` must pass in CI
- `rustfmt` formatting required
- All public API items (functions, structs, enums, traits) must have doc comments (`///`)
- Error handling: `anyhow::Result` for application-level, `thiserror::Error` for library-level custom errors
- No `unwrap()` or `expect()` in production code paths (only in tests)
- All `TODO` / `FIXME` must be tracked in tasks.md

#### Scenario: CI 检查
- **WHEN** 提交代码触发 CI
- **THEN** `cargo clippy -- -D warnings` 和 `cargo fmt -- --check` 必须通过

#### Scenario: 公开 API 无文档
- **WHEN** 公开函数/结构体缺少 doc comment
- **THEN** `cargo doc` 生成警告，CI 标记为需修复

### Requirement: 信任链规格

The system SHALL implement a three-tier trust chain:
1. **Root Trust**: Official directory signing key fingerprints hardcoded in hypo binary, with 90-day rotation transition period
2. **Developer Trust**: Developer GPG key fingerprints registered in official directory shards (SHA256-protected)
3. **Package Trust**: .hypo package GPG signature + manifest signature + per-file SHA256 hashes

#### Scenario: 根信任验证
- **WHEN** 客户端拉取 registry.json + registry.sig
- **THEN** 用硬编码公钥指纹验证签名，过渡期检查 `old_key_retired_at` 是否已过期

#### Scenario: 分片完整性验证
- **WHEN** 客户端拉取开发者分片 JSON
- **THEN** 计算分片 SHA256，与 registry.json 中 `shard_hashes` 对比，不一致则拒绝

#### Scenario: 包签名验证失败
- **WHEN** .hypo.sig 验证失败或 manifest.toml.sig 验证失败或任一文件哈希不匹配
- **THEN** 拒绝执行安装脚本，输出清晰错误，退出码非 0

### Requirement: 包格式规格

The .hypo package SHALL be a ZIP archive with nupkg-style internal layout:
- `manifest.toml` at root: package metadata, script paths, sandbox declarations, dependency declarations, file hashes
- `tools/` directory: install/uninstall/update scripts + pre/post hooks
- `content/` directory: additional resources

#### Scenario: 包内结构验证
- **WHEN** 解包 .hypo 文件后检查结构
- **THEN** manifest.toml 必须存在于根目录，tools/install.<ext> 必须存在，content/ 可选

### Requirement: CLI 规格与退出码

The CLI SHALL use clap derive mode with standardized exit codes:
- `0`: Success
- `1`: Generic error
- `2`: CLI argument parsing error
- `10`: Signature verification failure
- `11`: Hash mismatch
- `12`: Network error
- `13`: Registry not found / package not found
- `14`: Freeze violation (attempting to install frozen version without --force)
- `15`: Downgrade detected (latest_version < latest_seen_version without --force)

#### Scenario: 签名验证失败退出码
- **WHEN** .hypo 包签名验证失败
- **THEN** 进程退出码为 10，stderr 输出签名验证错误详情

#### Scenario: 包未找到
- **WHEN** `hypo install @owner/pkg` 中 @owner 不在官方目录中
- **THEN** 进程退出码为 13，stderr 输出 "developer not found in official directory"

### Requirement: 脚本解释器规格

The system SHALL support multiple script interpreters via the `[interpreter].type` field in manifest.toml:
- `powershell`: Windows primary (`.ps1` scripts)
- `bash`: Linux primary (`.sh` scripts)
- `zsh`: macOS primary (`.sh` scripts)
- `python`: Cross-platform fallback (`.py` scripts)

MVP SHALL implement PowerShell only; other interpreters are trait stubs (`todo!()`).

#### Scenario: 选择解释器
- **WHEN** manifest.toml 中 `[interpreter].type = "python"`
- **THEN** hypo 调用系统 python 执行 tools/install.py

### Requirement: 依赖与版本规格

The system SHALL use SemVer 2.0.0 for versioning with prerelease tags. Dependency constraints SHALL support `>=`, `>`, `<=`, `<`, `=`, `^`, `~` operators. The lockfile (hypo.lock) SHALL lock exact versions + download URLs + SHA256 hashes.

#### Scenario: 版本约束解析
- **WHEN** manifest.toml 声明 `"@bob/utils >= 2.0.0"`
- **THEN** hypo 从 registry 中选取满足 `>= 2.0.0` 的最高版本

#### Scenario: 循环依赖检测
- **WHEN** 依赖树中存在 A → B → A 的循环
- **THEN** 拒绝安装，输出循环依赖链路

### Requirement: 安全防护规格

The system SHALL implement the following security protections in MVP:
- **Downgrade protection**: SQLite records `latest_seen_version` per package; if registry returns lower version, warn and require `--force`
- **Freeze three-state behavior**: no version → auto-install rollback + warn; version without `--force` → error; version with `--force` → warn + install
- **TOFU for --from-url**: non-GitHub-Pages URLs require manual fingerprint confirmation on first use
- **Sandbox declaration**: manifest `[sandbox]` field must exist (shown in `hypo info`, not enforced in MVP)

#### Scenario: 降级攻击防护
- **WHEN** registry 返回的 latest_version 低于本地 latest_seen_version
- **THEN** 打印 warn 提示可能的降级攻击，要求 `--force` 才继续

#### Scenario: 冻结版本自动回退
- **WHEN** 安装时不指定版本，且最新版本 freeze=true
- **THEN** 自动安装 rollback_version 并打印 warn
