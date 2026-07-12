# hypo 开发 TODO

> 基于 [SPEC.md](./SPEC.md) 与 [roadmap.md](../roadmap.md) 编制。阶段一细化到任务级，阶段二至四按特性粒度展开。

---

## 阶段一：MVP（最小可行产品）

### 目标

打通核心信任链路：**拉取两层 registry 索引 → 下载 .hypo 包 → 验证 GPG 双签 → 解包 → 执行安装脚本**。Windows 平台优先。

### MVP 模块架构总览

```
src/
├── main.rs                      # clap CLI 入口，子命令分发，#![forbid(unsafe_code)]
├── lib.rs                       # 测试用 re-export，#![forbid(unsafe_code)]
├── error.rs                     # thiserror 自定义错误类型 + 退出码映射
├── constants.rs                 # 硬编码官方公钥指纹、官方目录 URL 等
├── paths.rs                     # ~/.hypo/ 目录结构管理
├── config.rs                    # config.toml 读写
├── commands/                    # CLI 子命令实现
│   ├── mod.rs
│   ├── init.rs                  # hypo init
│   ├── install.rs               # hypo install（含 --from-url）
│   ├── uninstall.rs             # hypo uninstall
│   ├── list.rs                  # hypo list
│   ├── info.rs                  # hypo info
│   ├── registry.rs              # hypo registry add/remove/list/export
│   └── config_cmd.rs            # hypo config get/set
├── registry/                    # 两层 Registry 拉取与缓存
│   ├── mod.rs
│   ├── types.rs                 # serde 结构体：RegistryJson, ShardJson, HypoIndex, HypoPackage
│   ├── client.rs                # HTTP 拉取 registry.json/shards/index/package-index
│   ├── cache.rs                 # ETag/snapshot_version 缓存
│   └── trust.rs                 # --from-url 信任模型（GitHub Pages / TOFU）
├── crypto/                      # GPG 签名验证
│   ├── mod.rs
│   ├── verify.rs                # sequoia 签名验证（registry.sig / .hypo.sig / manifest.toml.sig）
│   ├── keyring.rs               # 本地 keyring 缓存（~/.hypo/keyring/）
│   └── github.rs                # GitHub GPG Keys API 公钥拉取
├── package/                     # .hypo 包处理
│   ├── mod.rs
│   ├── reader.rs                # .hypo ZIP 解包 + PackageReader trait
│   ├── manifest.rs              # manifest.toml 解析与校验
│   └── hash.rs                  # SHA256 逐文件哈希校验
├── executor/                    # 脚本执行
│   ├── mod.rs
│   ├── trait.rs                 # ScriptExecutor trait
│   └── powershell.rs            # Windows PowerShell 执行器
├── sandbox/                     # 沙箱（MVP 空实现）
│   ├── mod.rs
│   └── trait.rs                 # PlatformSandbox trait
├── db/                          # 本地 SQLite 数据库
│   ├── mod.rs
│   ├── schema.rs                # 表结构定义
│   └── operations.rs            # CRUD 操作
└── deps/                        # 依赖解析
    ├── mod.rs
    ├── resolver.rs              # 版本解析 + 循环检测（petgraph）
    └── lockfile.rs              # hypo.lock 生成与解析（在线模式）
```

---

### Step 0: 项目初始化与骨架

**涉及文件**：`Cargo.toml`、`src/main.rs`、`src/lib.rs`、`.gitignore`、`rustfmt.toml`、`clippy.toml`

**前置依赖**：无

- [ ] 0.1 执行 `cargo init`，配置 `Cargo.toml`：edition 2021、name = "hypo"、全部 MVP 依赖（clap、reqwest、tokio、sequoia-openpgp、serde、serde_json、toml、sha2、semver、zip、petgraph、anyhow、thiserror、tracing、tracing-subscriber、indicatif、directories、rusqlite、dialoguer）
- [ ] 0.2 配置 `.gitignore`：`/target`、`/Cargo.lock`（bin 项目保留 lock，按需）、`~/.hypo/` 模拟目录
- [ ] 0.3 配置 `rustfmt.toml`：edition 2021、最大宽度 100
- [ ] 0.4 创建 `src/main.rs` 与 `src/lib.rs`，顶部添加 `#![forbid(unsafe_code)]`
- [ ] 0.5 创建所有模块目录与 `mod.rs` 空文件（commands/、registry/、crypto/、package/、executor/、sandbox/、db/、deps/）
- [ ] 0.6 验证 `cargo build` 通过、`cargo clippy -- -D warnings` 零警告

**验证**：`cargo build && cargo clippy -- -D warnings && cargo fmt -- --check` 全部通过

---

### Step 1: 核心数据结构与 Trait 定义

**涉及文件**：`src/error.rs`、`src/constants.rs`、`src/paths.rs`、`src/registry/types.rs`、`src/executor/trait.rs`、`src/sandbox/trait.rs`、`src/package/reader.rs`（trait 部分）

**前置依赖**：Step 0

- [ ] 1.1 `src/error.rs`：用 `thiserror::Error` 定义 `HypoError` 枚举，包含变体：SignatureVerification、HashMismatch、Network、RegistryNotFound、PackageNotFound、FreezeViolation、DowngradeDetected、Database、Config、Io，每个变体映射到 SPEC 5.2 定义的退出码
- [ ] 1.2 `src/constants.rs`：硬编码官方目录 URL、官方签名公钥指纹集合（占位值，后续替换真实指纹）、默认配置文件名等常量
- [ ] 1.3 `src/paths.rs`：使用 `directories` crate 定位 `~/.hypo/` 目录，提供函数获取 config_path / cache_dir / keyring_dir / tmp_dir / db_path / logs_dir
- [ ] 1.4 `src/registry/types.rs`：定义所有 serde 结构体（对应 SPEC 7.1-7.4）：
  - `RegistryJson`（schema_version, snapshot_version, shards, shard_hashes, official_key_fingerprints, key_rotation, key_update_url, mirrors）
  - `ShardJson`（github_username, gpg_key_fingerprints, base_pkg_url, registered_at）
  - `HypoIndex`（schema_version, owner, packages[]）
  - `HypoIndexPackage`（name, description, repo, latest_version, versions[]）
  - `HypoIndexVersion`（version, released_at, package_index_path, manifest_path, manifest_sig_path, freeze, freeze_reason, rollback_version, hypo_deps, system_deps）
  - `HypoPackage`（schema_version, name, version, packages[]）
  - `HypoPackageEntry`（platform, arch, url, size, sha256, sig_url）
- [ ] 1.5 `src/package/manifest.rs`：定义 `Manifest` 结构体（对应 SPEC 7.5），含 `[package]`、`[scripts]`、`[interpreter]`、`[sandbox]`、`[dependencies]`、`[hashes]` 各段
- [ ] 1.6 `src/executor/trait.rs`：定义 `ScriptExecutor` trait，方法 `async fn execute(&self, script_path: &Path, env_vars: HashMap<String, String>) -> Result<ExitStatus>`
- [ ] 1.7 `src/sandbox/trait.rs`：定义 `PlatformSandbox` trait，方法 `fn is_available(&self) -> bool`、`fn audit_write(&self, path: &str) -> Result<()>`（MVP 空实现返回 Ok）
- [ ] 1.8 `src/package/reader.rs`：定义 `PackageReader` trait，方法 `fn read_manifest(&self) -> Result<Manifest>`、`fn extract_to(&self, dest: &Path) -> Result<()>`、`fn list_files(&self) -> Vec<String>`
- [ ] 1.9 `src/config.rs`：定义 `Config` 结构体（config.toml schema）：trusted_users、custom_registries、keyring_path、cache_dir、log_level
- [ ] 1.10 验证 `cargo build` 通过，所有类型可序列化/反序列化

**验证**：`cargo build` 通过，`cargo test` 中能为每个结构体写 serde round-trip 测试

---

### Step 2: Crypto 模块（sequoia 签名验证 PoC）

**涉及文件**：`src/crypto/verify.rs`、`src/crypto/keyring.rs`、`src/crypto/github.rs`

**前置依赖**：Step 1

- [ ] 2.1 `src/crypto/verify.rs`：使用 `sequoia-openpgp` 实现 `verify_signature(data: &[u8], sig: &[u8], cert: &Cert) -> Result<()>`，支持 detached signature 验证
- [ ] 2.2 `src/crypto/verify.rs`：实现 `verify_registry_sig(registry_json: &[u8], registry_sig: &[u8], trusted_fingerprints: &[String]) -> Result<()>`，用硬编码指纹验证官方目录签名
- [ ] 2.3 `src/crypto/verify.rs`：实现公钥轮换过渡期逻辑：检查 `key_rotation.old_key_retired_at`，未过期则 `registry.sig.old` 也接受，过期后仅接受 `registry.sig`
- [ ] 2.4 `src/crypto/verify.rs`：实现 `verify_hypo_sig(hypo_data: &[u8], sig_data: &[u8], developer_fingerprints: &[String]) -> Result<()>`
- [ ] 2.5 `src/crypto/verify.rs`：实现 `verify_manifest_sig(manifest: &[u8], sig: &[u8], developer_fingerprints: &[String]) -> Result<()>`
- [ ] 2.6 `src/crypto/keyring.rs`：实现本地 keyring 缓存——`save_cert(fingerprint: &str, cert: &Cert)`、`load_cert(fingerprint: &str) -> Option<Cert>`、`list_certs() -> Vec<String>`，存储在 `~/.hypo/keyring/`
- [ ] 2.7 `src/crypto/github.rs`：使用 `reqwest` 调用 GitHub GPG Keys API（`GET /users/{username}/gpg_keys`），解析响应并返回公钥指纹列表
- [ ] 2.8 `src/crypto/github.rs`：实现公钥获取优先级：本地 keyring 缓存 → 官方目录分片 → GitHub API 拉取
- [ ] 2.9 编写单元测试：用测试 GPG 密钥生成签名，验证 `verify_signature` 正确接受有效签名、拒绝篡改数据

**验证**：`cargo test crypto` 全部通过，能验证一个测试签名

---

### Step 3: Registry 拉取与缓存模块

**涉及文件**：`src/registry/client.rs`、`src/registry/cache.rs`、`src/registry/trust.rs`

**前置依赖**：Step 1、Step 2

- [ ] 3.1 `src/registry/client.rs`：实现 `fetch_registry_json() -> Result<(RegistryJson, Vec<u8>)>`，拉取官方目录 `registry.json` + `registry.sig`（+ `registry.sig.old` 过渡期）
- [ ] 3.2 `src/registry/client.rs`：调用 `crypto::verify::verify_registry_sig` 验证签名，失败则返回 `HypoError::SignatureVerification`（退出码 10）
- [ ] 3.3 `src/registry/client.rs`：实现 `fetch_shard(owner: &str) -> Result<ShardJson>`，按 owner 首字母拼 URL 拉取分片，计算 SHA256 与 `registry.json` 中 `shard_hashes` 对比，不匹配返回 `HypoError::HashMismatch`（退出码 11）
- [ ] 3.4 `src/registry/client.rs`：实现 `fetch_hypo_index(base_pkg_url: &str) -> Result<HypoIndex>`
- [ ] 3.5 `src/registry/client.rs`：实现 `fetch_hypo_package(base_pkg_url: &str, pkg: &str, ver: &str) -> Result<HypoPackage>`
- [ ] 3.6 `src/registry/client.rs`：实现 `fetch_manifest(base_pkg_url: &str, pkg: &str, ver: &str) -> Result<(Vec<u8>, Vec<u8>)>`，拉取 `manifest.toml` + `manifest.toml.sig`
- [ ] 3.7 `src/registry/cache.rs`：实现 HTTP 缓存——存储 ETag / Last-Modified 头，下次请求带条件头
- [ ] 3.8 `src/registry/cache.rs`：实现 `snapshot_version` 增量检测——本地缓存版本号与远端对比，相同则跳过分片拉取
- [ ] 3.9 `src/registry/trust.rs`：实现 `--from-url` 混合信任模式：
  - GitHub Pages URL（`*.github.io`）：自动信任，公钥从 GitHub GPG Keys API 拉取
  - 其他 URL：TOFU 模式，首次安装显示公钥指纹，用 `dialoguer` 等待用户确认后缓存到 keyring
- [ ] 3.10 编写集成测试：mock HTTP 服务器返回测试 registry 数据，验证完整拉取+验证流程

**验证**：`cargo test registry` 全部通过

---

### Step 4: Package 下载与解包模块

**涉及文件**：`src/package/reader.rs`、`src/package/manifest.rs`、`src/package/hash.rs`

**前置依赖**：Step 1

- [ ] 4.1 `src/package/reader.rs`：实现 `HypoPackageReader` 结构体，封装下载的 `.hypo` 文件路径
- [ ] 4.2 `src/package/reader.rs`：实现 `download(url: &str, dest: &Path) -> Result<()>`，使用 `reqwest` + `indicatif` 进度条下载
- [ ] 4.3 `src/package/reader.rs`：实现 `extract_to(&self, dest: &Path) -> Result<()>`，使用 `zip` crate 解包到临时目录
- [ ] 4.4 `src/package/manifest.rs`：实现 `parse(toml_str: &str) -> Result<Manifest>`，使用 `toml` crate 解析
- [ ] 4.5 `src/package/manifest.rs`：实现 `validate(&self) -> Result<()>`，校验 manifest 完整性（install 脚本必填、`[sandbox]` 段必须存在、`[hashes]` 段必须存在）
- [ ] 4.6 `src/package/manifest.rs`：实现包内 manifest 与 gh-pages manifest 一致性对比
- [ ] 4.7 `src/package/hash.rs`：实现 `compute_sha256(path: &Path) -> Result<String>`
- [ ] 4.8 `src/package/hash.rs`：实现 `verify_files(extract_dir: &Path, manifest: &Manifest) -> Result<()>`，逐个计算包内文件 SHA256 与 manifest `[hashes]` 对比，不匹配返回 `HypoError::HashMismatch`
- [ ] 4.9 编写单元测试：创建测试 .hypo 包（ZIP + manifest.toml + 脚本），验证解包+哈希校验流程

**验证**：`cargo test package` 全部通过

---

### Step 5: 脚本执行器（Windows PowerShell）

**涉及文件**：`src/executor/powershell.rs`、`src/executor/trait.rs`

**前置依赖**：Step 1

- [ ] 5.1 `src/executor/powershell.rs`：实现 `PowerShellExecutor` 结构体，实现 `ScriptExecutor` trait
- [ ] 5.2 `src/executor/powershell.rs`：使用 `tokio::process::Command` 调用 `powershell.exe -ExecutionPolicy Bypass -File <script>`
- [ ] 5.3 `src/executor/powershell.rs`：注入环境变量 `HYPO_CONTENT_DIR`（指向 content/ 目录）、`HYPO_PKG_NAME`、`HYPO_PKG_VERSION`、`HYPO_INSTALL_DIR`
- [ ] 5.4 `src/executor/powershell.rs`：捕获 stdout / stderr / exit code，exit code 非 0 时返回错误
- [ ] 5.5 `src/executor/powershell.rs`：安装前用 `dialoguer` 显示包信息（名称、版本、sandbox 声明）并等待用户确认
- [ ] 5.6 为 bash/zsh/python 解释器创建 trait stub（`todo!()`），编译通过但运行时 panic
- [ ] 5.7 编写测试：执行一个简单的 `install.ps1`（如 `Write-Host "installing"`），验证 stdout 捕获与环境变量注入

**验证**：`cargo test executor` 通过，能执行一个测试 PowerShell 脚本

---

### Step 6: 本地 SQLite 数据库

**涉及文件**：`src/db/schema.rs`、`src/db/operations.rs`

**前置依赖**：Step 1

- [ ] 6.1 `src/db/schema.rs`：定义 `packages` 表结构：id（PK）、owner、name、version、platform、arch、install_path、script_dir、source_registry、installed_at、latest_seen_version
- [ ] 6.2 `src/db/schema.rs`：定义 `registries` 表结构：id（PK）、name、base_pkg_url、is_official、added_at
- [ ] 6.3 `src/db/schema.rs`：实现 `init_db(path: &Path) -> Result<()>`，创建表（IF NOT EXISTS）
- [ ] 6.4 `src/db/schema.rs`：实现 `migrate(db: &Connection) -> Result<()>`，版本迁移逻辑
- [ ] 6.5 `src/db/operations.rs`：实现 `insert_package(db, pkg: &PackageRecord) -> Result<()>`
- [ ] 6.6 `src/db/operations.rs`：实现 `get_package(db, owner: &str, name: &str) -> Result<Option<PackageRecord>>`
- [ ] 6.7 `src/db/operations.rs`：实现 `list_packages(db) -> Result<Vec<PackageRecord>>`
- [ ] 6.8 `src/db/operations.rs`：实现 `delete_package(db, owner: &str, name: &str) -> Result<()>`
- [ ] 6.9 `src/db/operations.rs`：实现 `update_latest_seen_version(db, owner: &str, name: &str, version: &str) -> Result<()>`
- [ ] 6.10 `src/db/operations.rs`：实现 `get_latest_seen_version(db, owner: &str, name: &str) -> Result<Option<String>>`
- [ ] 6.11 编写单元测试：内存 SQLite（`:memory:`），验证全部 CRUD 操作

**验证**：`cargo test db` 全部通过

---

### Step 7: 依赖解析与 Lockfile

**涉及文件**：`src/deps/resolver.rs`、`src/deps/lockfile.rs`

**前置依赖**：Step 1、Step 3

- [ ] 7.1 `src/deps/resolver.rs`：实现版本约束解析——解析 `"@bob/utils >= 2.0.0"` 为 `(owner, name, operator, version)` 元组，支持 `>=`、`>`、`<=`、`<`、`=`、`^`、`~` 操作符
- [ ] 7.2 `src/deps/resolver.rs`：使用 `semver` crate 实现版本约束匹配 `satisfies(version: &Version, constraint: &Constraint) -> bool`
- [ ] 7.3 `src/deps/resolver.rs`：实现版本选择策略——从 registry 拉取所有版本，取满足约束的最高版本
- [ ] 7.4 `src/deps/resolver.rs`：使用 `petgraph` 构建依赖图，实现拓扑排序
- [ ] 7.5 `src/deps/resolver.rs`：实现循环依赖检测——若拓扑排序失败（图中存在环），返回错误并输出循环链路
- [ ] 7.6 `src/deps/resolver.rs`：实现 `resolve(root_pkg: &str, root_ver: &str) -> Result<Vec<ResolvedDep>>`，递归解析整个依赖树
- [ ] 7.7 `src/deps/lockfile.rs`：定义 `Lockfile` 结构体（TOML 格式）：记录每个依赖的确切版本、下载 URL、SHA256、来源 registry
- [ ] 7.8 `src/deps/lockfile.rs`：实现 `generate(resolved_deps: &[ResolvedDep]) -> Result<String>`，生成 lockfile 内容
- [ ] 7.9 `src/deps/lockfile.rs`：实现 `parse(toml_str: &str) -> Result<Lockfile>`
- [ ] 7.10 `src/deps/lockfile.rs`：实现在线模式完整性——用 lockfile 安装时回查 registry，URL/SHA256 与当前 `hypo-package.json` 不一致则拒绝并提示重新解析
- [ ] 7.11 编写单元测试：构造依赖树（A → B → C），验证解析、循环检测、lockfile 生成与解析

**验证**：`cargo test deps` 全部通过

---

### Step 8: CLI 命令串联

**涉及文件**：`src/commands/*.rs`、`src/config.rs`、`src/main.rs`

**前置依赖**：Step 1-7 全部

- [ ] 8.1 `src/main.rs`：用 clap derive 定义 CLI 结构，映射 SPEC 5.1 全部子命令 + SPEC 5.3 全局参数（`--verbose`/`--quiet`/`--no-color`/`--config`）
- [ ] 8.2 `src/main.rs`：实现退出码映射——`HypoError` 转 `process::exit(code)`，对应 SPEC 5.2 退出码表
- [ ] 8.3 `src/commands/init.rs`：实现 `hypo init`——创建 `~/.hypo/` 目录结构（config.toml、cache/、keyring/、tmp/、hypo.db、registries.toml），初始化 SQLite 数据库
- [ ] 8.4 `src/commands/install.rs`：实现 `hypo install @owner/pkg[@<ver>] [--force]` 完整流程：
  1. 拉取官方目录 registry.json + 验签
  2. 拉取开发者分片 + 哈希校验
  3. 拉取 hypo-index.json
  4. 确定 版本（指定版本用指定版，不指定用 latest_version）
  5. **降级防护**：对比本地 `latest_seen_version`，低于则 warn + 要求 `--force`（退出码 15）
  6. **Freeze 三态**：若目标版本 freeze=true，不指定版本→自动回退 rollback_version + warn；指定版本无 `--force`→error（退出码 14）；指定版本 + `--force`→warn + 继续
  7. 拉取 hypo-package.json，选择当前平台条目
  8. 下载 .hypo + .hypo.sig
  9. 验 .hypo.sig 整体签名
  10. 解包
  11. 拉取 manifest.toml + manifest.toml.sig，验签
  12. 对比包内 manifest 与 gh-pages manifest 一致性
  13. 逐文件 SHA256 校验
  14. 递归解析 hypo 依赖（Step 7）
  15. 用 `dialoguer` 显示包信息 + sandbox 声明，等待用户确认
  16. 执行 install 脚本（PowerShell）
  17. 写入本地数据库，更新 `latest_seen_version`
  18. 生成/更新 hypo.lock
- [ ] 8.5 `src/commands/install.rs`：实现 `hypo install --from-url <url> [--force]`——使用 `registry::trust` 模块建立信任，后续流程同 8.4
- [ ] 8.6 `src/commands/uninstall.rs`：实现 `hypo uninstall @owner/pkg`——从数据库查已安装版本，找到 uninstall 脚本路径，执行卸载，删除数据库记录
- [ ] 8.7 `src/commands/list.rs`：实现 `hypo list`——从数据库查询全部已安装包，表格输出（owner/name、版本、平台、来源 registry）
- [ ] 8.8 `src/commands/info.rs`：实现 `hypo info @owner/pkg`——拉取 hypo-index.json，展示所有版本、是否冻结、依赖、sandbox 声明、签名指纹、来源
- [ ] 8.9 `src/commands/registry.rs`：实现 `hypo registry add <name> <url>` / `remove <name>` / `list` / `export <file>`——操作 config.toml 中的 custom_registries 字段
- [ ] 8.10 `src/commands/registry.rs`：实现包名冲突策略——官方目录与自定义 registry 同名包，先验证内容一致性（版本与公钥指纹一致则无冲突）；不一致则提示用户选择来源（`--source <registry>`）
- [ ] 8.11 `src/commands/config_cmd.rs`：实现 `hypo config get/set <key> [val]`——操作 config.toml 字段
- [ ] 8.12 验证 clap 生成的 `--help` 输出覆盖全部子命令

**验证**：`cargo build` 通过，`hypo --help` 显示全部子命令

---

### Step 9: 集成测试与验收

**涉及文件**：`tests/` 目录

**前置依赖**：Step 1-8 全部

- [ ] 9.1 创建测试用 GPG 密钥对，生成测试 .hypo 包（含 manifest.toml + install.ps1 + 签名）
- [ ] 9.2 搭建 mock HTTP 服务器（或用本地文件系统模拟），提供测试 registry.json + 分片 + hypo-index + hypo-package + manifest + .hypo 包
- [ ] 9.3 端到端测试：`hypo init` → `hypo install @test-owner/test-pkg` → 验证安装脚本执行 → `hypo list` 显示已安装 → `hypo uninstall` 卸载
- [ ] 9.4 测试 `--from-url` GitHub Pages 模式（mock `*.github.io` URL）
- [ ] 9.5 测试 `--from-url` TOFU 模式（非 GitHub Pages URL，验证用户确认流程）
- [ ] 9.6 测试签名验证失败场景（篡改 .hypo.sig → 退出码 10）
- [ ] 9.7 测试哈希不匹配场景（篡改包内文件 → 退出码 11）
- [ ] 9.8 测试网络错误场景（mock 服务器返回 500 → 退出码 12）
- [ ] 9.9 测试包未找到场景（不存在的 @owner → 退出码 13）
- [ ] 9.10 测试 Freeze 三态行为（构造 freeze=true 的版本，验证三种场景）
- [ ] 9.11 测试降级防护（本地记录 latest=2.0，registry 返回 1.5 → 退出码 15）
- [ ] 9.12 测试循环依赖检测（构造 A→B→A 的测试数据）
- [ ] 9.13 测试 lockfile 生成与在线模式回查
- [ ] 9.14 逐项检查 MVP 验收标准（见下方清单）

### MVP 验收标准清单

- [ ] `hypo init` 能在 Windows 上创建 `~/.hypo/` 目录结构（config.toml、cache/、keyring/、tmp/、hypo.db、registries.toml）
- [ ] hypo 二进制中硬编码官方目录签名公钥指纹集合，能验证 `registry.sig` 对 `registry.json` 的签名（过渡期新旧密钥任一有效）
- [ ] 官方目录签名验证失败时拒绝拉取开发者信息并输出明确错误
- [ ] 开发者分片 JSON 的 SHA256 与 `registry.json` 中 `shard_hashes` 不一致时拒绝使用
- [ ] `hypo install @owner/pkg` 能从官方目录分片定位开发者 → 拉取 hypo-index → 拉取 hypo-package.json → 下载 .hypo 包 → 双签验证 → 解包 → 执行 install 脚本 → 写入本地数据库
- [ ] GPG 验证使用 `sequoia-openpgp` 纯 Rust 实现，Windows 上无需安装 Gpg4win
- [ ] `hypo install --from-url <github-pages-url>`：GitHub Pages URL 自动信任 GitHub 身份，公钥从 GitHub API 拉取
- [ ] `hypo install --from-url <other-url>`：非 GitHub Pages URL 走 TOFU，首次安装显示公钥指纹，用户确认后缓存到 keyring
- [ ] `hypo registry add/remove/list` 能管理本地自定义 registry 列表
- [ ] `hypo registry export <file>` 能导出本地注册表为文件
- [ ] 同一 `@owner/pkg` 同时存在于官方目录和自定义 registry 时，先验证内容一致性；不一致则提示用户选择来源
- [ ] `.hypo` 包整体签名验证失败时拒绝执行并输出清晰错误（退出码 10）
- [ ] manifest 签名验证失败时拒绝执行
- [ ] 包内文件 SHA256 与 manifest `[hashes]` 记录不一致时拒绝执行
- [ ] 公钥验证优先级正确：官方目录指纹 → 本地缓存 → GitHub API
- [ ] 当前平台无对应 `.hypo` 包时（hypo-package.json 中无匹配平台）输出明确错误
- [ ] `hypo install` 能生成 `hypo.lock` 文件，记录依赖树确切版本与下载信息
- [ ] 存在 `hypo.lock` 时安装优先使用 lockfile，确保可重现安装
- [ ] lockfile 在线模式：安装时回查 registry，URL/SHA256 不一致则拒绝并提示重新解析
- [ ] 循环依赖被检测并报告清晰错误
- [ ] **降级攻击防护**：registry 返回的 `latest_version` 低于本地 `latest_seen_version` 时打印 warn 并要求 `--force`
- [ ] manifest `[sandbox]` 字段必须存在，`hypo info` 能展示其内容供用户查看
- [ ] 冻结版本：无版本号安装时自动回退到 `rollback_version` 并打印 warn
- [ ] 冻结版本：指定版本无 `--force` 时 error 退出并提示回退命令
- [ ] 冻结版本：`--force` 时 warn 后执行安装
- [ ] `hypo list` 能列出已安装包及完整包名（含 owner）、版本、平台、来源 registry
- [ ] `hypo uninstall @owner/pkg` 能调用 `tools/uninstall.ps1` 完成卸载
- [ ] 离线场景：本地 keyring 有缓存且 registry 已缓存时仍可安装（阶段二增加签名 lockfile 离线模式）

---

## 阶段二：安全加固

### 目标

实现 freeze 熔断完整闭环、rollback 回退命令、脚本行为审计（事后告警）、gh-pages 索引更新、hypo doctor 健康检查、lockfile 签名与离线模式。

### 任务清单

- [ ] **任务 1：Freeze 熔断完整闭环**
  - [ ] 1.1 实现 `hypo -r -f <ver>` 命令：通过 GitHub Contents API 修改 gh-pages 上 hypo-index.json 中目标版本的 freeze/freeze_reason/rollback_version
  - [ ] 1.2 强制校验：freeze=true 时 freeze_reason 与 rollback_version 必填，否则拒绝提交
  - [ ] 1.3 rollback_version 必须是同一包已存在的非冻结版本

- [ ] **任务 2：Rollback 回退命令**
  - [ ] 2.1 实现 `hypo rollback @owner/pkg`：从本地数据库取上一安装版本，重新拉取脚本并执行
  - [ ] 2.2 实现 `hypo rollback @owner/pkg --to <ver>`：指定版本回退
  - [ ] 2.3 回退前校验目标版本未被冻结，回退后更新本地数据库版本记录

- [ ] **任务 3：脚本行为审计（权限分级策略）**
  - [ ] 3.1 管理员权限：启用 ETW 全文件系统监控 + DNS 查询日志（通过 `windows` crate）
  - [ ] 3.2 普通用户权限：降级为 FileSystemWatcher + 环境变量 `HYPO_AUDIT_WRITE` 注入
  - [ ] 3.3 启动时检测权限级别，审计日志中标注当前能力范围
  - [ ] 3.4 文件写入审计：管理员用 ETW，普通用户用 FileSystemWatcher + 环境变量
  - [ ] 3.5 网络请求审计：管理员用 ETW DNS 日志，普通用户仅记录 manifest 声明的 allowed_network_egress
  - [ ] 3.6 行为日志写入 `~/.hypo/logs/audit-<pkg>-<ver>-<timestamp>.log`，开头标注权限级别
  - [ ] 3.7 违规告警：写入 allowed_write_paths 外路径时事后打印 warn 并标记可疑行为

- [ ] **任务 4：gh-pages 索引更新（GitHub Contents API + PAT 加密存储）**
  - [ ] 4.1 使用 GitHub Contents API（`PUT /repos/{owner}/{repo}/contents/{path}`）更新 gh-pages 索引文件
  - [ ] 4.2 更新顺序：先版本目录内文件，最后更新 hypo-index.json（入口最后改）
  - [ ] 4.3 SHA 机制：先 GET 获取当前 blob SHA，再 PUT 带该 SHA，串行处理避免冲突
  - [ ] 4.4 失败重试：PUT 幂等，中途失败留下孤儿文件无害，重试同一版本号即可覆盖
  - [ ] 4.5 PAT 加密存储：使用 `keyring` crate 调用系统凭据存储（Windows Credential Manager / macOS Keychain / Linux Secret Service）
  - [ ] 4.6 gh CLI 作为可选后端（配置 `release_backend = "gh-cli"`）
  - [ ] 4.7 发版前校验 PAT 权限（至少 repo scope）

- [ ] **任务 5：hypo doctor 健康检查**
  - [ ] 5.1 检查 sequoia-openpgp 可用性
  - [ ] 5.2 检查 keyring 完整性（公钥指纹列表）
  - [ ] 5.3 检查根信任公钥指纹是否与硬编码一致
  - [ ] 5.4 检查公钥轮换过渡期状态（old_key_retired_at 是否已过期）
  - [ ] 5.5 检查网络连通性（GitHub API、registry 端点）
  - [ ] 5.6 检查本地数据库一致性（含 latest_seen_version 字段）
  - [ ] 5.7 检查审计日志模块可用性
  - [ ] 5.8 检查系统凭据存储可用性（PAT 是否可读写）
  - [ ] 5.9 检测 PowerShell 执行策略：AllSigned 被组策略阻止时提示缓解措施

- [ ] **任务 6：lockfile 签名与离线模式**
  - [ ] 6.1 实现 `hypo lock --sign`：用用户 GPG 密钥（sequoia）签名 hypo.lock，生成 hypo.lock.sig
  - [ ] 6.2 实现 `hypo lock --verify`：验证 lockfile 签名，确认签名者公钥在本地 keyring 中
  - [ ] 6.3 离线安装模式：已签名 lockfile 可跳过 registry 回查，直接使用 lockfile 中 URL/SHA256 下载（仍验证 .hypo 包签名与 manifest 哈希）
  - [ ] 6.4 安全保障：lockfile 签名者公钥不在本地 keyring 中时拒绝离线安装

### 阶段二验收标准

- [ ] `hypo -r -f v1.2.3 -r "RCE 漏洞 CVE-XXXX" --rollback-to v1.2.2` 能通过 GitHub Contents API 修改 gh-pages 索引并推送
- [ ] freeze 字段不完整时拒绝提交并报错
- [ ] `hypo rollback @owner/pkg` 能回退到上一版本，本地数据库同步更新
- [ ] `hypo rollback @owner/pkg --to v1.2.2` 能指定版本回退
- [ ] 管理员权限下：脚本执行后审计日志记录所有文件写入路径与网络请求（ETW）
- [ ] 普通用户权限下：审计降级为 FileSystemWatcher + 环境变量注入，日志标注能力范围
- [ ] 脚本写入 allowed_write_paths 外路径时事后打印 warn 并标记可疑行为
- [ ] 脚本访问 allowed_network_egress 外域名时事后告警
- [ ] manifest 中 allowed_write_paths 被篡改后验签失败
- [ ] `hypo doctor` 能输出各项健康检查结果，问题项标红
- [ ] 审计日志文件格式清晰可读，开头标注权限级别
- [ ] GitHub PAT 能正确存储和读取，发版前校验权限
- [ ] gh CLI 可选后端可通过配置切换
- [ ] `hypo lock --sign` 能签名 lockfile，`hypo lock --verify` 能验证签名
- [ ] 已签名的 lockfile 可离线安装（跳过 registry 回查，但仍验证 .hypo 包签名）
- [ ] lockfile 签名者公钥不在本地 keyring 中时拒绝离线安装

---

## 阶段三：生态完善

### 目标

实现 `hypo -r` 自动化发版、镜像源自动探测与切换、系统包依赖检查、跨平台补齐、search/info 完善、hypo-updater 自更新模块。

### 任务清单

- [ ] **任务 1：hypo -r 自动发版（GitHub API）**
  - [ ] 1.1 首次交互式向导：采集版本号、脚本路径、资源目录、目标平台、签名密钥、审计配置、changelog
  - [ ] 1.2 保存 per-repo 工作流文件 `<repo>/.hypo/release.workflow.toml`
  - [ ] 1.3 `--auto` 模式读取 workflow.toml 自动完成全流程
  - [ ] 1.4 发版流程：收集文件 → 计算哈希 → 打包 .hypo → sequoia 签名 → GitHub Releases API 上传 → Contents API 更新 gh-pages
  - [ ] 1.5 发版前校验：.hypo 包内 manifest 与 gh-pages manifest 一致性检查
  - [ ] 1.6 多平台发版：一次为多平台分别打包上传
  - [ ] 1.7 CI/CD 集成：workflow.toml 可在 GitHub Actions 中通过 `hypo -r --auto` 调用

- [ ] **任务 2：镜像源自动探测与切换**
  - [ ] 2.1 官方目录镜像树：读取 registry.json 中 mirrors 字段
  - [ ] 2.2 开发者 registry 镜像：`hypo registry add-mirror <name> <url>` 管理
  - [ ] 2.3 并发 HEAD 请求探测 TTFB，按延迟排序缓存最优镜像（带 TTL）
  - [ ] 2.4 故障转移：当前镜像失败自动切换到下一个

- [ ] **任务 3：系统包依赖检查**
  - [ ] 3.1 manifest 中 `required_system_deps` 声明系统包
  - [ ] 3.2 Windows：检查 Get-Command / 注册表卸载项
  - [ ] 3.3 Linux：检查 dpkg -l / rpm -q
  - [ ] 3.4 macOS：检查 brew list
  - [ ] 3.5 缺失时提示用户手动安装，`--skip-system-deps` 跳过检查

- [ ] **任务 4：跨平台补齐**
  - [ ] 4.1 实现 Linux bash 脚本执行器
  - [ ] 4.2 实现 macOS zsh 脚本执行器
  - [ ] 4.3 补齐 Linux/macOS 行为审计（inotify 文件监控、DNS 查询日志）
  - [ ] 4.4 CI 矩阵：Windows + Ubuntu + macOS 三平台测试

- [ ] **任务 5：hypo search / hypo info 完善**
  - [ ] 5.1 定义 `SearchBackend` trait（async fn search），实现 `FullScanBackend`（全量遍历 + 并发 + 缓存）
  - [ ] 5.2 search：遍历所有已配置 registry 的开发者 hypo-index，模糊匹配包名/描述
  - [ ] 5.3 info：展示指定 @owner/pkg 的所有版本、冻结状态、依赖、签名指纹、来源 registry

- [ ] **任务 6：hypo-updater 自更新模块**
  - [ ] 6.1 独立模块 hypo-updater，通过 GitHub Releases API 检查新版本
  - [ ] 6.2 下载新二进制（带 GPG 签名验证，复用 sequoia）
  - [ ] 6.3 原子替换：下载到临时文件 → 验签 → 重命名替换
  - [ ] 6.4 `hypo self-update` 命令触发更新
  - [ ] 6.5 支持回退到上一版本（保留旧二进制备份）

### 阶段三验收标准

- [ ] `hypo -r` 首次运行走交互式向导，生成 `.hypo/release.workflow.toml`
- [ ] `hypo -r --auto` 读取 workflow.toml 完成全流程发版
- [ ] 发版通过 GitHub API 上传 .hypo 包与签名，无需 gh CLI
- [ ] 发版后 gh-pages 上索引文件正确更新
- [ ] 发版使用 sequoia-openpgp 签名，无需 Gpg4win
- [ ] workflow.toml 在 GitHub Actions 中可用 `hypo -r --auto` 调用
- [ ] 镜像探测：多镜像场景下自动选择延迟最低的镜像
- [ ] 镜像故障转移：主镜像不可用时自动切换备用
- [ ] 官方镜像与用户自配镜像并列存在于候选池
- [ ] manifest 声明系统依赖时，缺失依赖会提示用户
- [ ] `--skip-system-deps` 能跳过系统依赖检查
- [ ] Linux/macOS 上 `hypo install` 全流程通过
- [ ] Linux/macOS 行为审计正常工作
- [ ] `hypo search "tool"` 能返回模糊匹配结果
- [ ] `hypo info @owner/pkg` 能展示完整版本历史与冻结状态
- [ ] `hypo self-update` 能检查新版本、下载、验签、原子替换并支持回退

---

## 阶段四：高级特性

### 目标

Registry 分布式同步（Seed List）、严格沙箱（可选特性，事前拦截）、Windows PowerShell 深度支持、发版工作流高级特性。

### 任务清单

- [ ] **任务 1：Registry 分布式同步与 fork 生存能力**
  - [ ] 1.1 官方目录同步：Seed List 为官方目录仓库的多个 git remote，通过 git2 fetch 并发同步
  - [ ] 1.2 开发者 registry 同步：每个开发者 gh-pages 可配置多个 seed
  - [ ] 1.3 基于 commit GPG 签名验证 remote 可信度
  - [ ] 1.4 合并策略：冲突时以官方/原始 remote 优先
  - [ ] 1.5 `hypo registry add-seed <name> <git-url>` 管理种子节点
  - [ ] 1.6 `hypo sync` 命令触发全量同步
  - [ ] 1.7 fork 生存能力：`hypo config set official-dir <fork-url>` 指向 fork 继续运行

- [ ] **任务 2：严格沙箱（可选特性）**
  - [ ] 2.1 Windows：AppContainer + Job Object + 限制令牌 + WFP 网络过滤（通过 `windows` crate）
  - [ ] 2.2 Linux：Landlock + seccomp + namespaces（通过 `landlock`、`libseccomp`、`nix` crate）
  - [ ] 2.3 macOS：sandbox-exec 策略文件 + seatbelt
  - [ ] 2.4 沙箱配置模板化：manifest 中声明 `sandbox_profile`
  - [ ] 2.5 `hypo install --sandbox` 显式启用沙箱
  - [ ] 2.6 沙箱违规审计日志记录到 `~/.hypo/logs/sandbox-audit.log`

- [ ] **任务 3：Windows PowerShell 深度支持**
  - [ ] 3.1 PowerShell 执行策略处理：自动绕过 ExecutionPolicy，记录到审计日志
  - [ ] 3.2 PSModule 清单解析（可选增强）：解析 .psd1 文件提取模块元数据
  - [ ] 3.3 错误诊断：脚本失败时解析 `$Error[0]` 与 `InvocationInfo`，输出结构化错误位置

- [ ] **任务 4：发版工作流高级特性**
  - [ ] 4.1 prerelease 通道：`hypo -r --prerelease beta` 自动版本号递增
  - [ ] 4.2 发版前自动验证：脚本本地空跑（沙箱中 dry-run）、依赖树完整性检查
  - [ ] 4.3 批量发版：一次发布多个关联包

### 阶段四验收标准

- [ ] `hypo registry add-seed` 能为指定 registry 添加种子节点
- [ ] `hypo sync` 能从多个 seed 同步，冲突时以官方/原始 remote 优先
- [ ] 从第三方 seed 拉取的 registry 内容仍需通过 GPG 验签
- [ ] 用户能 fork 官方目录仓库并指向 fork 继续安装已注册包
- [ ] fork 后的官方目录不支持新开发者注册（预期行为）
- [ ] 配合已签名 lockfile 可完全离线安装
- [ ] `hypo install --sandbox` 能启用严格沙箱（事前拦截）
- [ ] 沙箱配置通过 manifest 中 `sandbox_profile` 字段声明
- [ ] 脚本违反 sandbox_profile 任意规则时被事前拦截并记录审计日志
- [ ] Windows 上 AppContainer 沙箱通过端到端测试（需管理员权限）
- [ ] Linux 上 Landlock + seccomp + namespaces 沙箱通过端到端测试
- [ ] .ps1 脚本执行失败时输出结构化错误信息（行号、错误类型、调用栈）
- [ ] `hypo -r --prerelease beta` 能自动递增 prerelease 版本号
- [ ] 发版前 dry-run 能在沙箱中空跑脚本并报告潜在问题

---

## 跨阶段依赖关系

```
阶段一 (MVP)
  Step 0: 项目骨架
    ↓
  Step 1: 数据结构 + Trait
    ↓                ↓              ↓             ↓           ↓
  Step 2: Crypto   Step 4: Package  Step 5: Executor  Step 6: DB  Step 7: Deps (需 Step 3)
    ↓                ↓              ↓             ↓           ↓
  Step 3: Registry  ←───────────────┴──────────────┴───────────┘
    ↓
  Step 8: CLI 串联 (需 Step 1-7 全部)
    ↓
  Step 9: 集成测试
    ↓
阶段二 (安全加固)
  依赖阶段一全部完成
    ↓
阶段三 (生态完善)
  依赖阶段二 lockfile 签名 + gh-pages 更新
    ↓
阶段四 (高级特性)
  依赖阶段三跨平台补齐 + 自更新
```

**关键依赖**：
- Step 2（crypto PoC）是最大技术风险点，应优先验证 sequoia API 可行性
- Step 3 依赖 Step 1 + Step 2
- Step 7 依赖 Step 3（需拉取 registry 数据解析依赖）
- Step 8 依赖 Step 1-7 全部完成
- 阶段二依赖阶段一 lockfile 在线模式
- 阶段三依赖阶段二 gh-pages 更新 + PAT 存储
- 阶段四依赖阶段三跨平台补齐
