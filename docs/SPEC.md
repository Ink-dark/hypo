# hypo 项目规格说明 (SPEC)

> **hypo (High-trust Repository Operator)** 是一个去中心化的通用软件安装/卸载/更新管理器。核心思想：不打包，只验证和执行开发者提供的安装脚本，并提供一键发版、依赖树解析等服务。安全性基于 GitHub 账号身份与 GPG 签名。
>
> **核心差异化**：hypo 是少数能在官方源消失后，用户 fork 一份依赖树即可继续运行的包管理器。去中心化 + 端到端签名 + 依赖树可 fork = 真正的生存能力。

---

## 一、项目定位

### 1.1 通用包管理器

hypo 是一个**通用软件安装/卸载/更新管理器**，支持分发任意编程语言、任意运行时的软件。它不是 Rust 专用工具，而是作为 winget、Scoop、Chocolatey 等中心化包管理器的去中心化替代方案。

hypo 的安装脚本可以是 PowerShell、Bash、Zsh 或 Python，由 manifest.toml 中的 `[interpreter].type` 字段声明。开发者可以分发：
- 编译型语言工具（C++、Go、Rust 编译的二进制）
- 解释型语言工具（Python、Node.js、Ruby 脚本）
- 混合型工具（含二进制 + 配置文件 + 脚本）
- 纯资源包（配置模板、数据集等）

### 1.2 与现有工具对比

| 维度 | winget | Scoop | Chocolatey | cargo-install | hypo |
|------|--------|-------|------------|---------------|------|
| 去中心化 | 否（微软控制源） | 否（社区 bucket 但中心化） | 否（Chocolatey 官方源） | 否（crates.io 中心化） | 是（fork 即可继续运行） |
| GPG 签名验证 | 否 | 否 | 否（有但可选） | 否（SHA 校验） | 是（双签 + 分片哈希） |
| 跨平台 | Windows only | Windows only | Windows only | 跨平台 | 跨平台（MVP Windows 优先） |
| 语言无关 | 是 | 是 | 是 | 否（仅 Rust） | 是 |
| 官方源消失后生存 | 否 | 否 | 否 | 否 | 是（fork 依赖树继续运行） |

---

## 二、编码标准

### 2.1 Rust 编码标准

- **Rust edition**：2021
- **工具链**：stable
- **格式化**：`rustfmt` 强制，CI 执行 `cargo fmt -- --check`
- **Lint**：`clippy` 以 `-D warnings` 级别运行，CI 中必须零警告通过
- **Edition lints**：启用所有 edition 2021 兼容 lints

### 2.2 严禁 unsafe 代码

**主代码库（src/ 下所有 .rs 文件）严禁出现 `unsafe` 关键字。**

所有需要 unsafe 操作的场景（FFI 调用、平台特定底层 API）必须通过经过审核的第三方 crate 间接调用：

| 需求 | 审核通过的 crate |
|------|-----------------|
| Windows API（ETW、AppContainer 等） | `windows` / `windows-sys` |
| Linux API（Landlock、seccomp 等） | `nix` / `landlock` |
| macOS API | `nix` / `core-foundation` |
| 加密操作 | `sequoia-openpgp` / `sha2` |
| ZIP 解压 | `zip` |

CI 应配置 `#![forbid(unsafe_code)]` 在 lib.rs / main.rs 顶部，编译期阻止 unsafe。

### 2.3 错误处理规范

- **应用层**（commands/、main.rs）：使用 `anyhow::Result`，错误向上传播用 `?`，用 `.context()` 添加上下文
- **库层**（crypto/、registry/、package/ 等被复用的模块）：使用 `thiserror::Error` 定义自定义错误类型
- **严禁**在生产代码路径中使用 `.unwrap()` 或 `.expect()`（仅测试代码允许）
- 错误输出到 stderr 时需包含：错误类型、错误消息、上下文链路

### 2.4 文档要求

- 所有 `pub` 项（函数、结构体、枚举、trait）必须有 `///` doc comment
- `cargo doc` 生成零警告
- 模块级别使用 `//!` 文档注释说明模块职责
- 复杂逻辑需在代码中添加 `//` 行注释说明 why（不是 what）

### 2.5 依赖审核标准

- 新增 dependency（Cargo.toml [dependencies]）必须在 tasks.md 中登记说明用途
- 禁止引入含 unsafe 且未经审计的 crate（除非无可替代，需在 tasks.md 中说明理由）
- 优先选择纯 Rust 实现的 crate（如 `sequoia-openpgp` 而非 `gpgme`，`rustls` 而非 `openssl`）
- dev-dependencies 不受此限制，但仍需登记

---

## 三、架构规格

### 3.1 模块结构

```
src/
├── main.rs                      # clap CLI 入口，子命令分发
├── lib.rs                       # 测试用 re-export，含 #![forbid(unsafe_code)]
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

### 3.2 信任链规格

三层信任模型：

```
根信任（硬编码公钥指纹）
    ↓ 验证 registry.sig
官方目录（registry.json + 分片哈希）
    ↓ 分片 SHA256 校验
开发者信任（分片中的 gpg_key_fingerprints）
    ↓ 验证 .hypo.sig + manifest.toml.sig
包信任（.hypo 包 + manifest 哈希）
    ↓ 逐文件 SHA256 校验
执行安装脚本
```

**根信任**：
- hypo 二进制中硬编码官方签名公钥指纹集合
- 公钥轮换过渡期固定 90 天，新旧密钥同时签名（`registry.sig` + `registry.sig.old`）
- `key_rotation.old_key_retired_at` 时间戳后拒绝旧密钥签名

**开发者信任**：
- 开发者 GPG 公钥指纹注册在官方目录分片中
- 分片完整性由 `registry.json` 中 `shard_hashes` 保护（SHA256）

**包信任**：
- `.hypo` 整体 GPG 签名（`.hypo.sig`）
- `manifest.toml` 独立签名（`manifest.toml.sig`，托管在 gh-pages）
- manifest 内记录包内所有文件 SHA256

### 3.3 包格式规格

`.hypo` 文件本质是 ZIP 压缩包，内部采用 nupkg 风格布局：

```
my-tool-1.2.3-windows.hypo (ZIP)
├── manifest.toml                 # 包元数据（含所有文件哈希）
├── tools/                        # 脚本目录
│   ├── install.ps1               # 安装脚本（必填）
│   ├── uninstall.ps1             # 卸载脚本（可选）
│   ├── update.ps1                # 更新脚本（可选）
│   ├── pre-install.ps1           # 安装前钩子（可选）
│   ├── post-install.ps1          # 安装后钩子（可选）
│   ├── pre-uninstall.ps1         # 卸载前钩子（可选）
│   ├── post-uninstall.ps1        # 卸载后钩子（可选）
│   ├── pre-update.ps1            # 更新前钩子（可选）
│   └── post-update.ps1           # 更新后钩子（可选）
└── content/                      # 附加资源目录（可选）
    ├── config-template.toml
    └── ...
```

验证顺序：下载 `.hypo` → 验 `.hypo.sig` 整体签名 → 解包 → 读 `manifest.toml` → 验 `manifest.toml.sig`（从 gh-pages 拉取）→ 逐个校验包内文件哈希与 manifest 记录一致。

### 3.4 Registry 结构规格

两层注册制：

**第一层：官方目录**（分片结构，托管在官方 GitHub Pages）
- `registry.json`：顶层索引，含分片列表 + 分片哈希表 + 快照版本 + 公钥轮换信息
- `registry.sig` / `registry.sig.old`：GPG 签名
- `{首字母}/{username}.json`：开发者分片

**第二层：开发者自有 registry**（各管各的包，托管在开发者 GitHub Pages）
- `hypo-index.json`：开发者所有包的顶层索引
- `{pkg}/{version}/hypo-package.json`：版本下载信息
- `{pkg}/{version}/manifest.toml` + `manifest.toml.sig`：版本 manifest + 签名

---

## 四、安全规格

### 4.1 GPG 双签机制

- **包整体签名**：`.hypo.sig` 对整个 `.hypo` 文件签名
- **manifest 签名**：`manifest.toml.sig` 对 manifest 独立签名（托管在 gh-pages，不在包内）
- **实现**：使用 `sequoia-openpgp` 纯 Rust crate，静态链接零外部依赖，Windows 上无需安装 Gpg4win
- **验证顺序**：先验整体签名 → 解包 → 验 manifest 签名 → 校验文件哈希

### 4.2 哈希校验规格

- **registry.json 分片哈希**：`shard_hashes` 字段包含每个分片文件的 SHA256，registry.json 已签名 → 分片哈希受保护 → 客户端拉取分片后校验
- **manifest 文件哈希**：`[hashes]` 段记录包内所有文件（tools/* 和 content/*）的 SHA256
- **hypo-package.json 包哈希**：`sha256` 字段记录 .hypo 文件的 SHA256

### 4.3 降级防护规格

- 本地 SQLite 记录每个包的 `latest_seen_version`
- 若 registry 返回的 `latest_version` 低于本地记录值，打印 warn 并要求 `--force` 才继续安装
- **合法降级路径**：开发者若需废弃高版本回退到低版本，应使用 freeze 机制（冻结高版本 + 设 `rollback_version`），不应直接修改 `latest_version`——后者会触发所有用户的降级警告

### 4.4 信任模型规格

`--from-url` 混合信任模式：
- **GitHub Pages URL**（`*.github.io`）：自动信任 GitHub 身份，公钥从 GitHub GPG Keys API 拉取
- **其他 URL**：走 TOFU（Trust On First Use），首次安装显示公钥指纹，用户手动确认后缓存到本地 keyring

### 4.5 Freeze 三态行为

| 场景 | 行为 |
|------|------|
| 不指定版本，最新版本 freeze=true | 自动安装 rollback_version + warn |
| 指定版本无 `--force` | error 退出并提示回退命令 |
| 指定版本 + `--force` | warn + 执行安装 |

### 4.6 沙箱声明

manifest `[sandbox]` 字段必须存在，声明脚本承诺的写入路径与网络出口：
- MVP 阶段：`hypo info` 展示供用户查看，不强制拦截
- 阶段二：用于行为审计对比（事后告警）
- 阶段四：配合 `--sandbox` flag 做事前拦截

---

## 五、CLI 规格

### 5.1 子命令集

```
hypo init                                            # 初始化本地配置与 keyring
hypo install @owner/pkg[@<ver>] [--force]            # 安装官方目录中的包
hypo install --from-url <url> [--force]              # 从自定义 URL 安装
hypo uninstall @owner/pkg                            # 卸载包
hypo update @owner/pkg                               # 更新包
hypo rollback @owner/pkg [--to <ver>]                # 回退版本
hypo search <keyword>                                # 搜索（遍历所有已配置的 registry）
hypo list                                            # 列出已安装包
hypo info @owner/pkg                                 # 查看包详情
hypo registry add <name> <base-pkg-url>              # 添加自定义 registry
hypo registry remove <name>                          # 移除自定义 registry
hypo registry list                                   # 列出已配置的 registry
hypo registry export <file>                          # 导出本地注册表
hypo config get/set <key> [val]                      # 管理全局配置
hypo doctor                                          # 环境健康检查
hypo -r [--auto]                                     # 发版（首次交互式，后续可 --auto）
hypo -r -f <ver>                                     # 冻结指定版本
```

### 5.2 退出码规范

| 退出码 | 含义 |
|--------|------|
| 0 | 成功 |
| 1 | 通用错误 |
| 2 | CLI 参数解析错误 |
| 10 | 签名验证失败 |
| 11 | 哈希不匹配 |
| 12 | 网络错误 |
| 13 | Registry 未找到 / 包未找到 |
| 14 | Freeze 违规（试图安装冻结版本但未加 --force） |
| 15 | 降级检测（latest_version < latest_seen_version 且未加 --force） |

### 5.3 全局参数

| 参数 | 说明 |
|------|------|
| `--verbose` / `-v` | 详细日志输出 |
| `--quiet` / `-q` | 静默模式（仅输出错误） |
| `--no-color` | 禁用彩色输出 |
| `--config <path>` | 指定配置文件路径 |

---

## 六、兼容性规格

### 6.1 平台支持

- **MVP**：Windows 优先，代码架构通过 trait 抽象预留跨平台能力
- **阶段三**：补齐 Linux（bash）和 macOS（zsh）支持
- 跨平台 trait：`PlatformSandbox`、`ScriptExecutor`、`KeyringBackend`、`PackageReader`

### 6.2 脚本解释器支持

通过 manifest.toml `[interpreter].type` 字段声明：

| 类型 | 平台 | 扩展名 | MVP 状态 |
|------|------|--------|---------|
| `powershell` | Windows | `.ps1` | 已实现 |
| `bash` | Linux | `.sh` | trait stub（`todo!()`） |
| `zsh` | macOS | `.sh` | trait stub（`todo!()`） |
| `python` | 跨平台 | `.py` | trait stub（`todo!()`） |

### 6.3 SemVer 合规

- 版本格式：SemVer 2.0.0（`MAJOR.MINOR.PATCH`）
- prerelease 标签：`-alpha`、`-beta`、`-rc.1` 等
- 约束操作符：`>=`、`>`、`<=`、`<`、`=`、`^`（兼容版本）、`~`（近似版本）
- lockfile 锁定确切版本 + 下载 URL + SHA256

---

## 七、数据结构规格

### 7.1 registry.json schema

```jsonc
{
  "schema_version": 1,
  "snapshot_version": 42,                     // 每次 PR 合并递增，用于增量同步
  "shards": ["a", "b", "c", "..."],           // 所有分片（首字母）列表
  "shard_hashes": {                           // 每个分片文件的 SHA256 哈希
    "a/alice.json": "sha256:abc123def456...",
    "b/bob.json": "sha256:ghi789jkl012..."
  },
  "official_key_fingerprints": [              // 官方签名公钥指纹（也硬编码在 hypo 中）
    "A1B2C3D4E5F6...",                        // 当前密钥
    "F6E5D4C3B2A1..."                         // 旧密钥（过渡期同时有效）
  ],
  "key_rotation": {                           // 公钥轮换过渡期管理
    "old_key_fingerprint": "F6E5D4C3B2A1...",
    "old_key_retired_at": "2026-10-01T00:00:00Z",
    "transition_period_days": 90
  },
  "key_update_url": "https://hypo-org.github.io/keys/current.json",
  "mirrors": [
    "https://mirror1.example.com/hypo-directory",
    "https://mirror2.example.com/hypo-directory"
  ]
}
```

### 7.2 开发者分片 JSON schema

```jsonc
{
  "github_username": "alice",
  "gpg_key_fingerprints": [
    "A1B2C3D4E5F6..."
  ],
  "base_pkg_url": "https://alice.github.io/hypo-pkgs",
  "registered_at": "2026-01-15T08:00:00Z"
}
```

### 7.3 hypo-index.json schema

```jsonc
{
  "schema_version": 1,
  "owner": "alice",
  "packages": [
    {
      "name": "my-tool",
      "description": "A cool CLI tool",
      "repo": "alice/my-tool",
      "latest_version": "1.2.3",
      "versions": [
        {
          "version": "1.2.3",
          "released_at": "2026-06-01T12:00:00Z",
          "package_index_path": "my-tool/1.2.3/hypo-package.json",
          "manifest_path": "my-tool/1.2.3/manifest.toml",
          "manifest_sig_path": "my-tool/1.2.3/manifest.toml.sig",
          "freeze": false,
          "freeze_reason": null,
          "rollback_version": null,
          "hypo_deps": ["@bob/utils >= 2.0.0"],
          "system_deps": []
        }
      ]
    }
  ]
}
```

### 7.4 hypo-package.json schema

```jsonc
{
  "schema_version": 1,
  "name": "my-tool",
  "version": "1.2.3",
  "packages": [
    {
      "platform": "windows",                  // windows / linux / macos / all
      "arch": ["x86_64", "aarch64"],
      "url": "https://github.com/alice/my-tool/releases/download/v1.2.3/my-tool-1.2.3-windows.hypo",
      "size": 245678,
      "sha256": "abc123def456...",
      "sig_url": "https://github.com/alice/my-tool/releases/download/v1.2.3/my-tool-1.2.3-windows.hypo.sig"
    }
  ]
}
```

### 7.5 manifest.toml schema

```toml
# manifest.toml（同时存在于 gh-pages 版本目录和 .hypo 包内）

[package]
name = "my-tool"
version = "1.2.3"
description = "A cool CLI tool"
author = "alice"
repo = "alice/my-tool"
platform = "windows"           # windows / linux / macos / all
arch = ["x86_64", "aarch64"]

[scripts]
install = "tools/install.ps1"
uninstall = "tools/uninstall.ps1"     # 可选
update = "tools/update.ps1"           # 可选
pre_install = "tools/pre-install.ps1" # 可选
post_install = "tools/post-install.ps1"
pre_uninstall = "tools/pre-uninstall.ps1"
post_uninstall = "tools/post-uninstall.ps1"
pre_update = "tools/pre-update.ps1"
post_update = "tools/post-update.ps1"

[interpreter]
type = "powershell"            # powershell / bash / zsh / python

[sandbox]
# 安全声明：脚本承诺的写入路径与网络出口
# MVP 阶段：必须存在，供用户安装前查看（hypo info 可展示），但不强制拦截
allowed_write_paths = [
  "$env:LOCALAPPDATA/my-tool",
  "$env:USERPROFILE/.hypo/bin"
]
allowed_network_egress = [
  "github.com",
  "api.github.com"
]

[dependencies]
hypo = [
  "@bob/utils >= 2.0.0",
]
system = []                    # 后续阶段启用

[hashes]                       # 包内所有文件的 SHA256（用于解包后校验）
"tools/install.ps1" = "abc123..."
"tools/uninstall.ps1" = "def456..."
"content/config-template.toml" = "ghi789..."
```

---

## 附录：官方依赖列表

| Crate | 阶段 | 用途 |
|-------|------|------|
| `clap` (v4, derive) | MVP | CLI 解析 |
| `reqwest` (rustls-tls) | MVP | HTTP 客户端 |
| `tokio` | MVP | 异步运行时 |
| `sequoia-openpgp` | MVP | 纯 Rust GPG 签名验证 |
| `serde` + `serde_json` + `toml` | MVP | 序列化与配置解析 |
| `sha2` | MVP | SHA256 哈希校验 |
| `semver` | MVP | 版本号解析与比较 |
| `zip` | MVP | .hypo 包解包 |
| `petgraph` | MVP | 依赖树拓扑排序 |
| `anyhow` + `thiserror` | MVP | 错误处理 |
| `tracing` + `tracing-subscriber` | MVP | 结构化日志 |
| `indicatif` | MVP | 下载进度条 |
| `directories` | MVP | 跨平台目录定位 |
| `rusqlite` (bundled) | MVP | 本地 SQLite 数据库 |
| `dialoguer` | MVP | 安装前确认提示 |
| `keyring` | 阶段二 | PAT 加密存储 |
| `windows` (windows-sys) | 阶段二 | ETW 审计、DPAPI |
| `regex` | 阶段二 | 路径白名单匹配 |
| `git2` | 阶段四 | 分布式 seed 同步 |
