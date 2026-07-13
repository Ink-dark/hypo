# CLAUDE.md

此文件为 Claude Code (claude.ai/code) 在本仓库中工作时提供指导。

## 项目概览

**hypo**（High-trust Repository Operator）是一个去中心化、跨平台的通用包管理器，用 Rust 编写。与 winget/Scoop/Chocolatey 不同，hypo 不打包软件——它只验证并执行开发者提供的安装脚本。安全性基于 GPG 信任链：硬编码根密钥指纹 → 已签名官方目录 → 开发者分片 → 已签名 `.hypo` 包 → 逐文件 SHA256 哈希。

核心差异化：即使官方源消失，用户 fork 一份依赖树即可继续安装已注册的包。

## 构建与检查命令

```bash
cargo build                  # 构建项目
cargo clippy -- -D warnings  # Lint 检查（必须零警告通过）
cargo fmt -- --check         # 格式化检查（max_width = 100）
cargo test                   # 运行全部测试
cargo test <模块名>           # 运行单个模块测试，如 cargo test crypto
cargo doc                    # 生成文档（必须零警告）
```

CI 完整检查：`cargo build && cargo clippy -- -D warnings && cargo fmt -- --check`

## 安全与编码规范（不可违反）

- **`#![forbid(unsafe_code)]`** 同时存在于 `src/main.rs` 和 `src/lib.rs`。所有 unsafe 操作必须通过经审核的第三方 crate 间接调用（`windows`、`nix`、`sequoia-openpgp` 等）。
- **生产代码禁止 `.unwrap()` 和 `.expect()`**，仅测试代码中允许使用。
- **库层**（crypto、registry、package 等被复用的模块）：使用 `thiserror::Error` 定义自定义错误类型。
- **应用层**（commands、main.rs）：使用 `anyhow::Result`，配合 `.context()` 添加上下文。
- **所有 `pub` 项**必须有 `///` 文档注释，模块级别使用 `//!` 文档注释说明模块职责。
- **新增依赖**优先选择纯 Rust 实现的 crate（如 `sequoia-openpgp` 而非 `gpgme`，`rustls` 而非 `openssl`）。

## 架构

### 当前状态

项目处于 **Step 0（项目骨架）阶段**。12 个模块均已创建占位文件，`Cargo.toml` 已声明全部 MVP 依赖。实现遵循 9 步 MVP 计划（详见 `docs/TODO.md`），后续还有三个阶段：安全加固、生态完善、高级特性。

### 模块结构图

```
src/
├── main.rs           # clap CLI 入口（Step 8），#![forbid(unsafe_code)]
├── lib.rs            # 测试用模块 re-export，#![forbid(unsafe_code)]
├── error.rs          # HypoError 枚举（thiserror），映射退出码 0-15
├── constants.rs      # 硬编码：官方目录 URL、根 GPG 公钥指纹集合
├── paths.rs          # ~/.hypo/ 目录结构管理（依赖 `directories` crate）
├── config.rs         # Config 结构体（config.toml），serde 序列化
├── commands/         # CLI 子命令实现：init, install, uninstall, list, info, registry, config_cmd
├── registry/         # 两层 Registry 拉取与缓存
│   ├── types.rs      # Serde 结构体：RegistryJson, ShardJson, HypoIndex, HypoPackage
│   ├── client.rs     # HTTP 拉取 + 签名验证 registry 数据
│   ├── cache.rs      # ETag/snapshot_version 条件缓存
│   └── trust.rs      # --from-url 信任模型（GitHub Pages 自动信任 / TOFU）
├── crypto/           # GPG 签名验证（sequoia-openpgp，纯 Rust）
│   ├── verify.rs     # 分离签名验证：registry.sig / .hypo.sig / manifest.toml.sig
│   ├── keyring.rs    # 本地 keyring 缓存（~/.hypo/keyring/）
│   └── github.rs     # GitHub GPG Keys API 公钥拉取
├── package/          # .hypo 包处理（.hypo = ZIP 压缩包，nupkg 风格布局）
│   ├── reader.rs     # PackageReader trait + 下载解包
│   ├── manifest.rs   # manifest.toml 解析与校验
│   └── hash.rs       # SHA256 逐文件哈希校验（对照 manifest [hashes]）
├── executor/         # 脚本执行（跨平台 trait 抽象）
│   ├── trait.rs      # ScriptExecutor trait
│   └── powershell.rs # Windows PowerShell 执行器（MVP）
├── sandbox/          # 沙箱（MVP 空实现；阶段四：AppContainer/Landlock/seccomp）
│   └── trait.rs      # PlatformSandbox trait
├── db/               # 本地 SQLite（rusqlite，bundled 模式）
│   ├── schema.rs     # 表结构：packages（含 latest_seen_version 降级防护）、registries
│   └── operations.rs # 已安装包与 registry 的 CRUD 操作
└── deps/             # 依赖解析
    ├── resolver.rs   # SemVer 约束解析 + petgraph 拓扑排序 + 循环检测
    └── lockfile.rs   # hypo.lock 生成与解析（TOML 格式）
```

### 信任链（核心安全模型）

```
硬编码根密钥指纹（编译在 hypo 二进制中）
  → 验证 registry.sig → 信任 registry.json
    → 分片 SHA256 与 shard_hashes 对比 → 信任开发者分片
      → 从分片提取开发者 GPG 指纹 → 验证 .hypo.sig + manifest.toml.sig
        → 逐文件 SHA256 与 manifest [hashes] 对比 → 执行安装脚本
```

公钥轮换：90 天过渡期，新旧密钥双签（`registry.sig` + `registry.sig.old`）。

### 关键 Trait（跨平台抽象）

- `ScriptExecutor` — 执行安装/卸载脚本（MVP 实现 PowerShell，bash/zsh/python 留 `todo!()`）
- `PlatformSandbox` — 沙箱隔离（MVP 空实现，阶段四实现真实沙箱）
- `PackageReader` — 读取/解包 .hypo 文件

### MVP 实现顺序（依赖关系严格）

各步骤必须按依赖顺序执行：

1. **Step 0** ✅ — 项目骨架（已完成）
2. **Step 1** — 数据结构 + Trait（error.rs、constants、paths、config、registry types、各 trait）
3. **Step 2** — Crypto 模块（sequoia 签名验证 PoC）——**最大技术风险点，应优先验证**
4. **Step 3** — Registry 拉取与缓存（依赖 Step 1 + 2）
5. **Step 4** — Package 下载与解包（依赖 Step 1）
6. **Step 5** — 脚本执行器（依赖 Step 1）
7. **Step 6** — SQLite 数据库（依赖 Step 1）
8. **Step 7** — 依赖解析与 lockfile（依赖 Step 3）
9. **Step 8** — CLI 命令串联（依赖 Step 1–7 全部）
10. **Step 9** — 集成测试与验收

### 退出码规范

| 退出码 | 含义 |
|--------|------|
| 0 | 成功 |
| 1 | 通用错误 |
| 2 | CLI 参数解析错误 |
| 10 | 签名验证失败 |
| 11 | 哈希不匹配 |
| 12 | 网络错误 |
| 13 | Registry / 包未找到 |
| 14 | Freeze 违规（试图安装冻结版本但未加 --force） |
| 15 | 降级检测（latest_version < latest_seen_version 且未加 --force） |

### 核心依赖速查

| Crate | 用途 |
|-------|------|
| `clap` (derive) | CLI 参数解析 |
| `tokio` | 异步运行时 |
| `reqwest` (rustls-tls) | HTTP 客户端 |
| `sequoia-openpgp` | 纯 Rust GPG 签名验证 |
| `serde` / `serde_json` / `toml` | 序列化与配置解析 |
| `sha2` | SHA256 哈希 |
| `semver` | 版本号解析与比较 |
| `zip` | .hypo 包解包 |
| `petgraph` | 依赖图拓扑排序 |
| `anyhow` + `thiserror` | 错误处理（应用层 + 库层） |
| `tracing` + `tracing-subscriber` | 结构化日志 |
| `indicatif` | 下载进度条 |
| `directories` | 跨平台目录定位 |
| `rusqlite` (bundled) | 本地 SQLite 数据库 |
| `dialoguer` | 用户交互确认提示 |

### 关键文档

- `docs/SPEC.md` — 完整项目规格说明（架构、安全、CLI、数据结构 schema）
- `docs/TODO.md` — MVP 分步实现计划（Step 0–9 细化到任务级）
- `roadmap.md` — 四阶段开发路线图（架构决策、crate 选型、风险分析）
