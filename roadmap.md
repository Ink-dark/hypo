# hypo (High-trust Repository Operator) 开发路线图

> 去中心化的 CLI 分发工具。核心思想：**不打包，只验证和执行开发者提供的安装脚本**，并提供一键发版、依赖树解析等服务。安全性基于 GitHub 账号身份与 GPG 签名。
>
> **核心差异化**：hypo 是少数能在官方源消失后，用户 fork 一份依赖树即可继续运行的包管理器。除 Rust 生态的 cargo 外，主流工具（npm、pip、Scoop、winget）均无法做到。去中心化 + 端到端签名 + 依赖树可 fork = 真正的生存能力。

---

## 一、核心架构决策（全阶段基线）

| 维度 | 决策 |
|------|------|
| 核心定位 | **去中心化生存能力**：官方源消失后，用户 fork 依赖树即可继续运行。区别于 Scoop（Windows-only 无签名）、cargo-install（Rust-only）、winget（中心化） |
| 整体架构 | 两层注册制：**官方目录**（轻量目录服务）+ **开发者自有 registry**（各管各的包），去中心化 |
| 官方目录 | 按 username 首字母分片（`a/alice.json`、`b/bob.json`），避免单文件瓶颈与 PR 冲突；`registry.json` 中包含各分片文件的 SHA256 哈希表，保证分片完整性；PR 审核制注册，后期 OAuth |
| 根信任机制 | 官方目录 JSON 本身 GPG 双签名（`registry.sig`），支持公钥轮换过渡期（新旧密钥同时签名，过渡期固定 90 天，`old_key_retired_at` 后拒绝旧密钥）；签名公钥发布到可公开查询且可更换的位置，hypo 内置公钥指纹用于验证 |
| 分片完整性 | `registry.json` 中包含每个分片文件的 SHA256 哈希表（`shard_hashes`），registry.json 已签名 → 分片哈希受保护 → 客户端拉取分片后校验哈希 |
| 开发者 registry | `base-pkg-url/hypo-index.json` 为顶层索引，与各包文件夹并列；每版本目录内含 `hypo-package.json`（下载信息）+ `manifest.toml` + `manifest.toml.sig` |
| gh-pages 角色 | **仅存索引**（告诉客户端东西在哪里），不是下载源；实际 `.hypo` 包文件托管在 GitHub Releases 或其他下载源 |
| 包格式 | `.hypo` 文件（ZIP 压缩包），nupkg 风格内部布局（`tools/` 放脚本、`content/` 放资源、`manifest.toml` 在根） |
| 多平台策略 | 单平台一个 `.hypo` 包，`hypo-package.json` 中声明各平台包的下载 URL；跨平台脚本可用 `platform: "all"` 发单包 |
| 生命周期钩子 | 全套 pre/post 钩子（install/uninstall/update），按需声明；MVP 仅实现 install + uninstall |
| GPG 实现 | 使用 `sequoia-openpgp`（纯 Rust），静态链接零依赖，从根本上解决 Windows 部署问题 |
| 签名机制 | 双签：`.hypo` 包整体 GPG 签名 + manifest 内含包内所有文件 SHA256 哈希（manifest 本身也签名托管在 gh-pages） |
| 信任链 | 根信任（官方目录签名公钥）→ 官方目录（开发者公钥指纹）→ 开发者签名（.hypo 包 + manifest） |
| 依赖解析 | 支持 `hypo.lock` lockfile（类似 Cargo.lock），锁定整个依赖树；**双模式完整性**：在线安装时回查 registry 验证 URL/SHA256 一致性，用户签名的 lockfile 可离线跳过回查 |
| 包名语法 | npm 风格 `@owner/pkg` 为主，兼容全限定 URL 形式；`@` 前缀对应 GitHub 用户名/组织名 |
| 发包方式 | `hypo -r` 生成 `.hypo` 包 → 通过 GitHub API（PAT 认证）上传到 GitHub Releases → 通过 GitHub Contents API 更新 gh-pages 索引（hypo-index.json + hypo-package.json + manifest） |
| GitHub API 认证 | 用户提供 GitHub Personal Access Token（PAT），用于 Releases 上传 + gh-pages 索引更新；gh CLI 作为可选后端 |
| --from-url 信任模型 | 混合模式：GitHub Pages URL 自动信任 GitHub 身份（公钥从 GitHub API 拉取）；其他 URL 走 TOFU（首次安装显示公钥指纹，用户手动确认后缓存到本地 keyring） |
| 非注册源安装 | 支持 `hypo install --from-url <url>` 直接从自定义 URL 安装；安装后提示是否添加为本地镜像；本地注册表可导出 |
| 包名冲突策略 | 官方目录与自定义 registry 同名包：先验证内容一致性（版本与公钥指纹一致则无冲突）；不一致则提示用户选择来源（`--source <registry>`） |
| 发版工具 | `hypo -r` 与安装器同二进制；首次交互式 → 保存 per-repo 工作流文件 → 后续 `--auto` 复用，可接入 CI/CD |
| Freeze 机制 | `hypo -r -f <ver>` 修改自己 gh-pages 上的 `hypo-index.json`：`freeze=true` + `freeze_reason` + `rollback_version` |
| Freeze 安装行为 | 三态：① 不指定版本→自动装 rollback_version + warn；② 指定版本无 `--force`→error 并提示回退命令；③ 指定版本 + `--force`→warn + 安装 |
| 脚本沙箱 | 阶段二为**行为审计**（记录写入路径、网络请求，事后告警）；阶段四才做严格沙箱（AppContainer + Landlock + seccomp）作为可选特性 |
| 自更新机制 | 独立 `hypo-updater` 模块，与主程序分离，避免鸡生蛋问题；通过 GitHub Releases 检查新版本 |
| 镜像源 | 官方目录镜像树（社区贡献）与用户自配镜像**并列共存**，本地延迟/可用性探测自动选优 |
| Rollback | Per-package 回退：`hypo rollback <pkg>` 回上一版，`--to <ver>` 指定版本 |
| 分布式同步 | Git-based（clone/fetch registry 镜像），Seed List 即 git remote 列表 |
| 版本方案 | SemVer 2.0.0 + prerelease 标签（`-alpha` / `-beta` / `-rc.1`） |
| MVP 平台 | Windows 优先，代码架构预留跨平台 trait，后续补齐 Linux/macOS |
| 全局配置 | `~/.hypo/config.toml`：信任用户列表 + 自定义 registry 列表 + 运行时配置（keyring 路径、缓存目录、日志级别） |

### CLI 子命令集（全阶段）

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

### Registry 两层结构 schema（草案）

> **重要原则**：GitHub Pages 上托管的是**索引**（告诉客户端"东西在哪里"），不是下载源。实际的 `.hypo` 包文件托管在 GitHub Releases 或其他下载源，gh-pages 只存索引 JSON、manifest 和签名文件。

#### 第一层：官方目录（分片结构 + 分片哈希 + 根信任双签名）

托管在官方 GitHub Pages，按 username 首字母分片，避免单文件瓶颈与 PR 冲突。`registry.json` 包含所有分片文件的 SHA256 哈希表，保证分片完整性。官方目录本身 GPG 双签名（公钥轮换过渡期新旧密钥同时签名）。

```
hypo-org.github.io/directory/
├── registry.sig                              # 当前快照的 GPG 签名（签名公钥指纹硬编码在 hypo 中）
├── registry.sig.old                          # 旧密钥的签名（过渡期双签名，可选）
├── registry.json                             # 顶层索引：分片列表 + 分片哈希表 + 快照版本
├── a/
│   ├── alice.json                            # 开发者 alice 的注册信息
│   └── alex.json
├── b/
│   └── bob.json
└── ...
```

```jsonc
// https://hypo-org.github.io/directory/registry.json
{
  "schema_version": 1,
  "snapshot_version": 42,                     // 每次合并 PR 递增，用于增量同步
  "shards": ["a", "b", "c", "..."],           // 所有分片（首字母）列表
  "shard_hashes": {                           // 每个分片文件的 SHA256 哈希（registry.json 已签名 → 分片哈希受保护）
    "a/alice.json": "sha256:abc123def456...",
    "a/alex.json": "sha256:def456abc123...",
    "b/bob.json": "sha256:ghi789jkl012..."
  },
  "official_key_fingerprints": [              // 官方签名公钥指纹（也硬编码在 hypo 中作为根信任）
    "A1B2C3D4E5F6...",                        // 当前密钥
    "F6E5D4C3B2A1..."                         // 旧密钥（过渡期同时有效）
  ],
  "key_rotation": {                           // 公钥轮换过渡期管理
    "old_key_fingerprint": "F6E5D4C3B2A1...",
    "old_key_retired_at": "2026-10-01T00:00:00Z",  // 旧密钥退役时间，客户端在此时间后拒绝旧密钥签名
    "transition_period_days": 90              // 过渡期固定 90 天
  },
  "key_update_url": "https://hypo-org.github.io/keys/current.json",  // 公钥轮换地址（新密钥需旧密钥签名）
  "mirrors": [
    "https://mirror1.example.com/hypo-directory",
    "https://mirror2.example.com/hypo-directory"
  ]
}
```

```jsonc
// https://hypo-org.github.io/directory/a/alice.json
{
  "github_username": "alice",
  "gpg_key_fingerprints": [
    "A1B2C3D4E5F6...",
    "F6E5D4C3B2A1..."
  ],
  "base_pkg_url": "https://alice.github.io/hypo-pkgs",
  "registered_at": "2026-01-15T08:00:00Z"
}
```

**根信任与分片完整性验证流程**：
1. hypo 二进制中硬编码官方签名公钥指纹集合（可随版本更新轮换）
2. 客户端拉取 `registry.json` + `registry.sig`（+ `registry.sig.old` 过渡期）
3. 用硬编码指纹对应的公钥验证 `registry.sig` 对 `registry.json` 的签名
4. **过渡期退出机制**：检查 `key_rotation.old_key_retired_at`，当前时间超过该时间戳则拒绝旧密钥签名（即使旧密钥已泄露，攻击者签名的恶意 registry.json 也会被拒绝）
5. 拉取开发者分片 JSON（如 `a/alice.json`）
6. 计算分片文件 SHA256，与 `registry.json` 中 `shard_hashes` 对比，**不一致则拒绝**
7. 公钥轮换：通过 `key_update_url` 拉取最新公钥列表（需旧密钥签名验证），过渡期固定 90 天
8. 分片哈希机制确保：即使 HTTPS 被 MITM，攻击者篡改分片后哈希不匹配，验证失败

#### 第二层：开发者自有 registry（hypo-index.json + 包目录）

每个开发者在自己的 GitHub Pages 上托管索引。`hypo-index.json` 与各包文件夹并列，每个包文件夹下按版本再套目录，版本目录内含 `hypo-package.json`（声明该版本的下载信息）和 `manifest.toml` + 签名。

```
alice.github.io/hypo-pkgs/
├── hypo-index.json                              # 开发者所有包的顶层索引
├── my-tool/
│   ├── 1.2.3/
│   │   ├── hypo-package.json                    # 该版本各平台包的下载信息
│   │   ├── manifest.toml                        # 该版本的 manifest（含哈希）
│   │   └── manifest.toml.sig                    # manifest 的 GPG 签名
│   └── 1.2.2/
│       ├── hypo-package.json
│       ├── manifest.toml
│       └── manifest.toml.sig
└── another-pkg/
    └── ...
```

```jsonc
// https://alice.github.io/hypo-pkgs/hypo-index.json
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
          "hypo_deps": [
            "@bob/utils >= 2.0.0",
            "@alice/common >= 1.0.0"
          ],
          "system_deps": []
        }
      ]
    }
  ]
}
```

```jsonc
// https://alice.github.io/hypo-pkgs/my-tool/1.2.3/hypo-package.json
// 声明该版本各平台 .hypo 包的下载位置（实际下载源，非 gh-pages）
{
  "schema_version": 1,
  "name": "my-tool",
  "version": "1.2.3",
  "packages": [
    {
      "platform": "windows",
      "arch": ["x86_64", "aarch64"],
      "url": "https://github.com/alice/my-tool/releases/download/v1.2.3/my-tool-1.2.3-windows.hypo",
      "size": 245678,
      "sha256": "abc123def456...",
      "sig_url": "https://github.com/alice/my-tool/releases/download/v1.2.3/my-tool-1.2.3-windows.hypo.sig"
    },
    {
      "platform": "linux",
      "arch": ["x86_64"],
      "url": "https://github.com/alice/my-tool/releases/download/v1.2.3/my-tool-1.2.3-linux.hypo",
      "size": 198234,
      "sha256": "def456abc123...",
      "sig_url": "https://github.com/alice/my-tool/releases/download/v1.2.3/my-tool-1.2.3-linux.hypo.sig"
    },
    {
      "platform": "all",  // 跨平台单包（如纯 Python/Rust 编译的脚本自己处理平台差异）
      "arch": ["x86_64", "aarch64"],
      "url": "https://github.com/alice/my-tool/releases/download/v1.2.3/my-tool-1.2.3-all.hypo",
      "size": 210456,
      "sha256": "ghi789jkl012...",
      "sig_url": "https://github.com/alice/my-tool/releases/download/v1.2.3/my-tool-1.2.3-all.hypo.sig"
    }
  ]
}
```

#### .hypo 包文件内部结构（nupkg 风格）

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
└── content/                      # 附加资源目录
    ├── config-template.toml
    ├── default-settings.json
    └── ...
```

#### 签名验证流程（双签）

1. **包整体签名**：`.hypo.sig` 对整个 `.hypo` 文件签名，客户端下载后先验整体签名
2. **manifest 哈希校验**：`manifest.toml` 内记录包内每个文件（tools/* 和 content/*）的 SHA256，manifest 本身也有独立签名（`manifest.toml.sig`，托管在 gh-pages）
3. **验证顺序**：下载 `.hypo` → 验 `.hypo.sig` 整体签名 → 解包 → 读 `manifest.toml` → 验 `manifest.toml.sig`（从 gh-pages 拉取）→ 逐个校验包内文件哈希与 manifest 记录一致

#### Manifest 文件（TOML）

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
# 阶段二：用于行为审计对比（事后告警）
# 阶段四：配合 --sandbox flag 做事前拦截
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

## 二、阶段一：MVP（最小可行产品）

### 目标
打通核心信任链路：**拉取两层 registry 索引 → 下载 .hypo 包 → 验证 GPG 双签 → 解包 → 执行安装脚本**。不包含网络探测、复杂沙箱、依赖树完整解析。Windows 平台优先验证。

### 关键技术任务

1. **项目骨架与 CLI 框架**
   - 使用 `clap` derive 模式定义子命令结构
   - 建立 `commands/` `registry/` `crypto/` `package/` `sandbox/` `executor/` 模块分层
   - 定义跨平台 trait：`PlatformSandbox`、`ScriptExecutor`、`KeyringBackend`、`PackageReader`
   - 实现 `hypo registry` 子命令（add / remove / list / export）

2. **两层 Registry 索引拉取与本地缓存**
   - 从官方 GitHub Pages 拉取 `registry.json` + `registry.sig`（+ `registry.sig.old` 过渡期），用硬编码公钥指纹验证根信任
   - 按 `@owner` 首字母拉取对应分片（如 `a/alice.json`）获取开发者 `base_pkg_url`
   - **分片完整性校验**：计算分片文件 SHA256，与 `registry.json` 中 `shard_hashes` 对比，不一致则拒绝
   - 拉取开发者 `hypo-index.json` + 按需拉取版本目录下的 `hypo-package.json` 和 `manifest.toml`
   - 实现 `hypo init` 初始化缓存目录与配置文件
   - 缓存失效策略：ETag / Last-Modified 头对比 + `snapshot_version` 增量检测
   - **`--from-url` 混合信任模式**：
     - GitHub Pages URL（`*.github.io`）：自动信任 GitHub 身份，公钥从 GitHub GPG Keys API 拉取
     - 其他 URL：走 TOFU（Trust On First Use），首次安装显示公钥指纹，用户手动确认后缓存到本地 keyring

3. **.hypo 包下载与解包**
   - 根据 `hypo-package.json` 中当前平台的 `url` 字段下载 `.hypo` 文件
   - ZIP 解包到临时目录（`~/.hypo/tmp/<pkg>-<ver>/`）
   - 解包后读取包内 `manifest.toml`，与 gh-pages 上的 `manifest.toml` 对比一致性
   - 下载 `.hypo.sig` 签名文件用于整体验签

4. **GPG 签名验证（sequoia + 双签流程）**
   - 使用 `sequoia-openpgp` 纯 Rust 实现，无需安装 Gpg4win 等外部依赖
   - 公钥来源优先级：官方目录指纹 → 本地 keyring 缓存 → GitHub GPG Keys API
   - 公钥缓存到本地 keyring（`~/.hypo/keyring/`），按指纹索引
   - **根信任**：验证官方目录 `registry.sig`（公钥指纹硬编码在 hypo 中）
   - **第一签**：验证 `.hypo.sig` 对整个 `.hypo` 包文件的签名
   - **第二签**：从 gh-pages 拉取 `manifest.toml.sig`，验证 manifest 签名
   - **哈希校验**：逐个计算包内文件（tools/*、content/*）的 SHA256，与 manifest `[hashes]` 表对比
   - 任一环节失败则拒绝执行

5. **脚本执行器（Windows 优先）**
   - Windows：调用 PowerShell 执行 `tools/install.ps1`，捕获 stdout/stderr/exit code
   - 根据 manifest 中的 `[interpreter].type` 字段选择执行器（powershell / bash / zsh / python）
   - 将 `content/` 目录路径通过环境变量（如 `HYPO_CONTENT_DIR`）暴露给脚本
   - 预留 Linux（bash）/ macOS（zsh）执行路径 trait
   - 执行前打印脚本摘要与签名指纹，要求用户确认（`--yes` 跳过）

6. **基础安装流程**
   - `hypo install @owner/pkg`：解析 `@owner` → 查官方目录分片 → 拉开发者 hypo-index → 取最新非冻结版本 → 拉 hypo-package.json → 下载 .hypo → 双签验证 → 解包 → 执行 install 脚本
   - `hypo install --from-url <url>`：直接从指定 base URL 拉取 hypo-index 并安装
   - 安装后提示是否将该源添加为本地 registry
   - 冻结版本的默认回退行为（自动装 rollback_version + warn）
   - **降级攻击防护**：本地 SQLite 记录每个包的 `latest_seen_version`，若 registry 返回的 `latest_version` 低于本地记录值，打印 warn 并要求 `--force` 才继续安装（防止 gh-pages 被入侵后回退到含已知漏洞的旧版本）
   - **合法降级路径**：开发者若需废弃高版本回退到低版本，应使用 freeze 机制（冻结高版本 + 设 `rollback_version`），不应直接修改 `latest_version`——后者会触发所有用户的降级警告
   - `hypo list` 列出已安装包（本地 SQLite 记录）

7. **依赖解析与 lockfile（在线模式）**
   - 解析 manifest `[dependencies].hypo` 中的版本约束（如 `@bob/utils >= 2.0.0`）
   - 版本选择策略：取满足约束的最高版本（参考 cargo resolver 简化版）
   - 递归解析依赖树，检测循环依赖（使用 `petgraph` 拓扑排序）
   - 生成 `hypo.lock` 文件：锁定整个依赖树的确切版本、下载 URL、SHA256
   - **在线模式完整性**：用 lockfile 安装时必须回查 registry，版本的 URL/SHA256 必须与当前 `hypo-package.json` 一致才使用；不一致则拒绝并提示重新解析
   - `hypo install` 优先使用 `hypo.lock`（如存在），确保可重现安装
   - `hypo install --update-deps` 忽略 lockfile 重新解析最新版本
   - lockfile 签名（`hypo lock --sign/--verify`）与离线模式推迟到阶段二，阶段一仅支持在线回查模式

8. **本地包数据库**
   - 使用 SQLite 记录已安装包：完整包名（含 owner）、版本、平台、安装时间、包 SHA256、来源 registry
   - **降级防护字段**：每个包记录 `latest_seen_version`，安装时对比 registry 返回值，低于本地记录则触发降级警告
   - 支持 `hypo uninstall` 调用 `tools/uninstall.ps1`（若存在）完成卸载
   - MVP 仅实现 install + uninstall 两个生命周期钩子

### Crate 选型

| Crate | 用途 |
|-------|------|
| `clap` (v4, derive) | CLI 解析 |
| `reqwest` (rustls-tls) | HTTP 客户端，拉取 registry 索引与下载 .hypo 包 |
| `tokio` | 异步运行时 |
| `sequoia-openpgp` | 纯 Rust GPG 签名验证，零外部依赖 |
| `serde` + `serde_json` + `toml` | 配置、registry 索引与 manifest 解析 |
| `sha2` | SHA256 哈希校验 |
| `semver` | 版本号解析与比较 |
| `zip` | .hypo 包（ZIP 格式）解包 |
| `petgraph` | 依赖树构建与拓扑排序 |
| `anyhow` + `thiserror` | 错误处理 |
| `tracing` + `tracing-subscriber` | 结构化日志 |
| `indicatif` | 下载进度条 |
| `directories` | 跨平台配置/缓存目录定位 |
| `rusqlite` (bundled) | 本地已安装包数据库（含 `latest_seen_version` 降级防护字段） |
| `keyring` | PAT 加密存储（调用系统凭据存储：Windows Credential Manager / macOS Keychain / Linux Secret Service） |
| `dialoguer` | 安装前确认提示 |

### 验收标准

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
- [ ] `.hypo` 包整体签名验证失败时拒绝执行并输出清晰错误（退出码非 0）
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
- [ ] 脚本能通过 `HYPO_CONTENT_DIR` 环境变量访问 `content/` 目录资源
- [ ] 代码中 `PlatformSandbox` / `ScriptExecutor` / `PackageReader` trait 已定义，Windows 实现就绪，Linux/macOS 留 `todo!()`

---

## 三、阶段二：安全加固

### 目标
实现 freeze 熔断的完整闭环、rollback 回退命令、**脚本行为审计**（事后告警而非事前拦截，严格沙箱推迟到阶段四）。

### 关键技术任务

1. **Freeze 熔断完整闭环**
   - `hypo -r -f <ver>` 命令：在当前仓库的 gh-pages 分支上修改 `hypo-index.json` 中目标版本的 `freeze`/`freeze_reason`/`rollback_version` → commit & push
   - 强制校验：`freeze=true` 时 `freeze_reason` 与 `rollback_version` 必填，否则拒绝提交
   - `rollback_version` 必须是同一包已存在的非冻结版本
   - Freeze 操作由包所有者在自己的仓库上执行，不涉及官方目录

2. **Rollback 回退命令**
   - `hypo rollback @owner/pkg`：从本地数据库取上一个安装版本，重新拉取该版本脚本并执行
   - `hypo rollback @owner/pkg --to <ver>`：指定版本回退
   - 回退前校验目标版本未被冻结
   - 回退后更新本地数据库版本记录

3. **脚本行为审计（非沙箱拦截，权限降级策略）**
   - **设计原则**：阶段二不做事前拦截，只做事后审计。严格沙箱（AppContainer/WFP/Landlock/seccomp）推迟到阶段四作为可选特性
   - **权限分级策略**：
     - **管理员权限**：启用 ETW 全文件系统监控 + DNS 查询日志，审计能力完整
     - **普通用户权限**：降级为 FileSystemWatcher + 脚本执行环境变量注入（仅监控 manifest 声明路径的写入），不监控全盘
     - 启动时检测权限级别，审计日志中标注当前能力范围
   - **文件写入审计**：管理员模式用 ETW，普通用户用 FileSystemWatcher + 环境变量 `HYPO_AUDIT_WRITE` 传递写入日志路径
   - **网络请求审计**：管理员模式用 ETW DNS 查询日志，普通用户模式仅记录 manifest 声明的 `allowed_network_egress`，超出则标记可疑
   - **行为日志**：所有审计记录写入 `~/.hypo/logs/audit-<pkg>-<ver>-<timestamp>.log`，日志开头标注权限级别
   - **违规告警**：脚本写入 manifest 声明的 `allowed_write_paths` 外路径时，事后打印 warn 并标记为"可疑行为"
   - **网络违规告警**：访问 `allowed_network_egress` 外域名时事后告警
   - 审计日志可用于阶段四沙箱策略的制定参考

4. **gh-pages 索引更新（GitHub Contents API + PAT 加密存储）**
   - 使用 GitHub Contents API（`PUT /repos/{owner}/{repo}/contents/{path}`）更新 gh-pages 上的索引文件
   - **更新顺序**：先更新版本目录内的文件（hypo-package.json / manifest.toml / manifest.toml.sig），最后更新 hypo-index.json（入口最后改）
   - **SHA 机制**：更新已存在文件需先 GET 获取当前 blob SHA，再 PUT 带该 SHA；并发更新会冲突，发版流程需串行处理 gh-pages 文件更新
   - **幂等性与失败重试**：Contents API 的 PUT 是幂等的（需带正确的 SHA）；若更新中途失败（如 hypo-index.json 更新失败），版本目录里可能留下孤儿文件，但无害（不被索引指向就不会被访问）；下次发版重试同一版本号即可覆盖
   - **PAT 加密存储**：使用 `keyring` crate 调用系统凭据存储（Windows Credential Manager / macOS Keychain / Linux Secret Service），PAT 不落盘明文
     - Windows：底层用 DPAPI（`CryptProtectData`，机器绑定，无需用户密码）
     - macOS：Keychain
     - Linux：Secret Service（需 D-Bus）
   - gh CLI 作为可选后端（用户配置 `release_backend = "gh-cli"` 时使用）
   - 发版前校验 PAT 权限（至少 `repo` scope）

5. **`hypo doctor` 健康检查**
   - 检查 sequoia-openpgp 可用性
   - 检查 keyring 完整性（公钥指纹列表）
   - 检查根信任公钥指纹是否与硬编码一致
   - 检查公钥轮换过渡期状态（`old_key_retired_at` 是否已过期）
   - 检查网络连通性（GitHub API、registry 端点）
   - 检查本地数据库一致性（含 `latest_seen_version` 降级防护字段）
   - 检查审计日志模块可用性
   - 检查系统凭据存储可用性（PAT 是否可读写）
   - 检测 PowerShell 执行策略：若系统级 `AllSigned` 策略被组策略阻止 `-ExecutionPolicy Bypass`，提示用户缓解措施（脚本需由受信任证书逐个签名，或联系管理员调整组策略）

6. **lockfile 签名与离线模式**
   - `hypo lock --sign`：用用户 GPG 密钥（sequoia）签名 `hypo.lock`，生成 `hypo.lock.sig`
   - `hypo lock --verify`：验证 lockfile 签名，确认签名者公钥已在本地 keyring 中
   - **离线安装模式**：已签名的 lockfile 可跳过 registry 回查，直接使用 lockfile 中记录的 URL/SHA256 下载 .hypo 包（仍验证 .hypo 包签名与 manifest 哈希）
   - 适用场景：气隙环境、企业内部镜像、fork 后官方源消失的生存场景
   - 安全保障：lockfile 签名者必须是用户已信任的公钥（在本地 keyring 中），否则拒绝

### Crate 选型（新增）

| Crate | 用途 |
|-------|------|
| `windows` (windows-sys) | ETW 文件系统审计、FileSystemWatcher、管理员权限检测、DPAPI |
| `keyring` | PAT 加密存储（系统凭据存储抽象层） |
| `regex` | 路径白名单匹配（审计对比） |

### 验收标准

- [ ] `hypo -r -f v1.2.3 -r "RCE 漏洞 CVE-XXXX" --rollback-to v1.2.2` 能通过 GitHub Contents API 修改 gh-pages 索引并推送
- [ ] freeze 字段不完整时（缺 reason 或 rollback_version）拒绝提交并报错
- [ ] `hypo rollback @owner/pkg` 能回退到上一版本，本地数据库同步更新
- [ ] `hypo rollback @owner/pkg --to v1.2.2` 能指定版本回退
- [ ] 管理员权限下：脚本执行后审计日志记录所有文件写入路径与网络请求（ETW）
- [ ] 普通用户权限下：审计降级为 FileSystemWatcher + 环境变量注入，日志标注能力范围
- [ ] 脚本写入 `allowed_write_paths` 外路径时事后打印 warn 并标记可疑行为
- [ ] 脚本访问 `allowed_network_egress` 外域名时事后告警
- [ ] manifest 中 `allowed_write_paths` 被篡改后验签失败
- [ ] `hypo doctor` 能输出各项健康检查结果，问题项标红
- [ ] 审计日志文件格式清晰可读，开头标注权限级别
- [ ] GitHub PAT 能正确存储和读取，发版前校验权限
- [ ] gh CLI 可选后端可通过配置切换
- [ ] `hypo lock --sign` 能用用户 GPG 密钥签名 lockfile，`hypo lock --verify` 能验证签名
- [ ] 已签名的 lockfile 可离线安装（跳过 registry 回查，但仍验证 .hypo 包签名）
- [ ] lockfile 签名者公钥不在本地 keyring 中时拒绝离线安装

---

## 四、阶段三：生态完善

### 目标
实现 `hypo -r` 自动化发版（通过 GitHub API）、镜像源自动探测与切换、系统包依赖检查、跨平台补齐、自更新模块。

### 关键技术任务

1. **`hypo -r` 自动发版（GitHub API）**
   - **首次交互式向导**：
     - 询问发版版本号（SemVer + prerelease）
     - 采集要发布的脚本路径（install / uninstall / update / pre-post 钩子）
     - 采集附加资源目录路径（content/）
     - 选择目标平台（windows / linux / macos / all）
     - 选择签名 GPG 密钥（列出本地可用密钥）
     - 声明审计配置（allowed_write_paths / allowed_network_egress）
     - 生成 changelog（从 git log 或手动输入）
   - **保存 per-repo 工作流**：将上述选择写入 `<repo>/.hypo/release.workflow.toml`
   - **后续 `--auto` 模式**：读取 workflow.toml，自动完成全流程
   - **发版流程**（全部通过 GitHub API，PAT 认证）：
     1. 收集脚本与资源，按 nupkg 风格组织临时目录（tools/ + content/ + manifest.toml）
     2. 计算 manifest 中 `[hashes]` 表（所有包内文件 SHA256）
     3. 打包为 `.hypo` 文件（ZIP 格式，文件名 `<pkg>-<ver>-<platform>.hypo`）
     4. 使用 `sequoia-openpgp` 签名 `.hypo` 文件生成 `.hypo.sig`
     5. 签名 `manifest.toml` 生成 `manifest.toml.sig`
     6. 通过 GitHub Releases API（`POST /repos/{owner}/{repo}/releases` + 上传资产）上传 `.hypo` 与 `.hypo.sig`
     7. 通过 GitHub Contents API（`PUT /repos/{owner}/{repo}/contents/{path}`）更新 gh-pages：追加 `hypo-package.json` + `manifest.toml` + `manifest.toml.sig` 到版本目录
     8. 通过 GitHub Contents API 更新 gh-pages 根 `hypo-index.json`（追加版本条目）
     9. 发版前校验：`.hypo` 包内 `manifest.toml` 与 gh-pages 上的 `manifest.toml` 一致性检查（否则双签验证会失败）
   - **gh CLI 可选后端**：用户可通过配置选择使用 gh CLI 作为上传后端（`release_backend = "gh-cli"`）
   - **多平台发版**：可一次为多个平台分别打包上传，`hypo-package.json` 中声明所有平台条目
   - **CI/CD 集成**：workflow.toml 可直接在 GitHub Actions 中通过 `hypo -r --auto` 调用

2. **镜像源自动探测与切换**
   - 官方目录镜像树：官方 `registry.json` 中声明的 `mirrors` 字段，用于加速官方目录拉取
   - 开发者 registry 镜像：用户可通过 `hypo registry add` 为特定开发者添加镜像
   - 用户自配镜像与官方镜像**并列合并**为一个候选池
   - 探测策略：
     - 并发 HEAD 请求各镜像（使用 `reqwest`，无需额外 HTTP 客户端），测量 TTFB
     - 探测实际可用性（HTTP 200 + 内容校验）
     - 按延迟排序，缓存最优镜像到本地（带 TTL）
   - 故障转移：当前镜像失败自动切换到下一个
   - `hypo registry add-mirror <name> <url>` 管理用户自定义镜像

3. **系统包依赖检查**
   - manifest 中 `required_system_deps` 声明系统包
   - Windows：检查 `Get-Command` / 注册表卸载项
   - Linux：检查 `dpkg -l` / `rpm -q`
   - macOS：检查 `brew list`
   - 缺失时提示用户手动安装，不自动安装（安全考量）
   - `--skip-system-deps` flag 可跳过检查

4. **跨平台补齐**
   - 实现 Linux bash 脚本执行器
   - 实现 macOS zsh 脚本执行器
   - 补齐 Linux/macOS 行为审计实现（inotify 文件监控、DNS 查询日志）
   - CI 矩阵：Windows + Ubuntu + macOS 三平台测试

5. **`hypo search` / `hypo info` 完善**
   - **`SearchBackend` trait 预留**：定义统一异步搜索接口（`async fn search(&self, keyword: &str) -> Vec<SearchResult>`），阶段三实现为 `FullScanBackend`（全量遍历），后期可替换为 `IndexBackend`（基于 search-index.json 集中索引），避免后期重构
   - search：遍历所有已配置 registry（官方目录分片 + 本地自定义）的开发者 hypo-index，模糊匹配包名/描述
   - 全量遍历 + 并发请求 + 本地缓存（带 TTL），规模上来后可切换为 `IndexBackend`
   - info：展示指定 `@owner/pkg` 的所有版本、是否冻结、依赖、签名指纹、来源 registry
   - `hypo info` 同时展示开发者 GitHub 用户名和 base_pkg_url

6. **hypo-updater 自更新模块**
   - 独立模块 `hypo-updater`，与主程序分离，避免鸡生蛋问题
   - 通过 GitHub Releases API 检查 hypo 自身新版本（`GET /repos/hypo-org/hypo/releases/latest`）
   - 下载新二进制（带 GPG 签名验证，复用 sequoia）
   - 原子替换：下载到临时文件 → 验签 → 重命名替换旧二进制
   - `hypo self-update` 命令触发更新检查与执行
   - 支持回退到上一版本（保留旧二进制备份）

### Crate 选型（新增）

| Crate | 用途 |
|-------|------|
| `fuzzy-matcher` | search 模糊匹配 |
| `sysinfo` | 系统信息采集（依赖检查辅助） |
| `self_update` 或自研 | hypo 自更新模块（基于 GitHub Releases） |

### 验收标准

- [ ] `hypo -r` 首次运行走交互式向导，结束后生成 `.hypo/release.workflow.toml`
- [ ] `hypo -r --auto` 读取 workflow.toml 完成全流程发版
- [ ] 发版通过 GitHub API 上传 `.hypo` 包与签名到 GitHub Releases，无需 gh CLI
- [ ] 发版后 gh-pages 上 `hypo-package.json`/`manifest.toml`/`manifest.toml.sig`/`hypo-index.json` 正确更新
- [ ] 发版使用 `sequoia-openpgp` 签名，无需 Gpg4win
- [ ] workflow.toml 在 GitHub Actions 中可用 `hypo -r --auto` 调用
- [ ] 镜像探测：多镜像场景下自动选择延迟最低的镜像
- [ ] 镜像故障转移：主镜像不可用时自动切换备用
- [ ] 官方镜像与用户自配镜像并列存在于候选池
- [ ] `hypo install @owner/pkg` 在 manifest 声明系统依赖时，缺失依赖会提示用户
- [ ] `--skip-system-deps` 能跳过系统依赖检查
- [ ] Linux/macOS 上 `hypo install` 全流程通过
- [ ] Linux/macOS 行为审计正常工作（文件写入、网络请求记录）
- [ ] `hypo search "tool"` 能返回模糊匹配结果（遍历官方目录分片 + 自定义 registry）
- [ ] `hypo info @owner/pkg` 能展示完整版本历史与冻结状态
- [ ] `hypo self-update` 能检查新版本、下载、验签、原子替换并支持回退

---

## 五、阶段四：高级特性

### 目标
Registry 分布式同步（Seed List）、**严格沙箱**（作为可选特性，从事后审计升级到事前拦截）、Windows PowerShell 深度支持。

### 关键技术任务

1. **Registry 分布式同步（Seed List）与 fork 生存能力**
   - 两层同步机制：官方目录同步 + 开发者 registry 同步
   - **官方目录同步**：Seed List 为官方目录仓库的多个 git remote，通过 `git fetch` 并发同步，按 commit 时间戳合并
   - **开发者 registry 同步**：每个开发者的 gh-pages 也可配置多个 seed（镜像仓库），同步策略同上
   - 基于 commit GPG 签名验证 remote 可信度（要求 remote 仓库的提交有可信密钥签名）
   - 合并策略：冲突时以官方/原始 remote 优先
   - `hypo registry add-seed <name> <git-url>` 管理种子节点
   - 显式 `hypo sync` 命令触发全量同步
   - 防篡改：registry JSON 本身仍要求 GPG 签名，即使从第三方 seed 拉取也需验签
   - **fork 生存能力**（核心差异化）：
     - 用户可 fork 整个官方目录仓库到自己的 GitHub，通过 `hypo config set official-dir <fork-url>` 指向 fork，即使官方源消失也能继续运行
     - **能力边界明确**：fork 保的是"已注册包的可安装性"，不是"生态的持续性"——新开发者无法加入、公钥轮换停止、已注册开发者的包仍能安装
     - 配合 `hypo.lock` lockfile，用户甚至可以完全离线运行（只要 .hypo 包已缓存）
     - 开发者 registry 同理，用户可 fork 单个开发者的 gh-pages 作为镜像源

2. **严格沙箱（可选特性，基于阶段二审计经验）**
   - **设计原则**：阶段二的行为审计数据为沙箱策略提供参考，阶段四实现事前拦截作为 `--sandbox` 可选 flag
   - 不依赖外部容器运行时，使用纯 Rust 实现的隔离原语：
     - **Windows**：AppContainer + Job Object + 限制令牌 + WFP 网络过滤，全部通过 `windows` crate 调用（注意 WFP 需管理员权限）
     - **Linux**：Landlock（文件系统）+ seccomp（系统调用）+ namespaces（mount/pid/net），使用 `landlock`、`libseccomp`、`nix` crate
     - **macOS**：sandbox-exec 策略文件 + seatbelt
   - 沙箱配置模板化：manifest 中声明 `sandbox_profile`（路径白名单、网络白名单、系统调用白名单）
   - `hypo install --sandbox` 显式启用沙箱（默认不启用，避免管理员权限要求阻断普通用户）
   - 沙箱违规审计日志：记录到 `~/.hypo/logs/sandbox-audit.log`
   - 可选：基于 `bpfbox` 或类似 Rust BPF 沙箱实验性集成

3. **Windows PowerShell 深度支持**
   - **脚本执行**：`.ps1` 脚本执行与 stdout/stderr/stderr 捕获（MVP 已具备）
   - **脚本验证**：GPG 签名 + SHA256 哈希双重验证（与平台无关，统一流程，使用 sequoia）
   - **PowerShell 执行策略处理**：自动绕过 `ExecutionPolicy`（通过 `-ExecutionPolicy Bypass` 参数），但记录到审计日志
   - **PSModule 清单解析**（可选增强）：解析 `.psd1` 文件提取模块元数据，作为 manifest 的补充
   - **错误诊断**：PowerShell 脚本失败时解析 `$Error[0]` 与 `InvocationInfo`，输出结构化错误位置

4. **发版工作流高级特性**
   - 支持 prerelease 通道：`hypo -r --prerelease beta` 自动版本号递增
   - 发版前自动验证：脚本本地空跑（在沙箱中 dry-run）、依赖树完整性检查
   - 批量发版：一次发布多个关联包（依赖树上下游一起发版）

### Crate 选型（新增）

| Crate | 用途 |
|-------|------|
| `nix` | Linux namespaces/unshare 操作 |
| `landlock` | Linux 文件系统沙箱（完整版） |
| `libseccomp` | Linux 系统调用过滤（完整版） |
| `windows` | AppContainer、WFP、限制令牌 |
| `git2` | Seed List 多 remote 同步 |

### 验收标准

- [ ] `hypo registry add-seed <name> <git-url>` 能为指定 registry 添加种子节点
- [ ] `hypo sync` 能从多个 seed 同步官方目录和开发者 registry，冲突时以官方/原始 remote 优先
- [ ] 从第三方 seed 拉取的 registry 内容仍需通过 GPG 验签
- [ ] 用户能 fork 官方目录仓库并通过 `hypo config set official-dir <fork-url>` 指向 fork 继续安装已注册包
- [ ] fork 后的官方目录不支持新开发者注册（预期行为，文档明确说明）
- [ ] 配合已签名 lockfile，用户可完全离线安装（registry 索引 + .hypo 包均已缓存）
- [ ] `hypo install --sandbox` 能启用严格沙箱（事前拦截）
- [ ] 沙箱配置通过 manifest 中 `sandbox_profile` 字段声明
- [ ] 脚本违反 `sandbox_profile` 任意规则时被事前拦截并记录审计日志
- [ ] `~/.hypo/logs/sandbox-audit.log` 包含完整违规记录
- [ ] Windows 上 AppContainer 沙箱通过端到端测试（需管理员权限）
- [ ] Linux 上 Landlock + seccomp + namespaces 沙箱通过端到端测试
- [ ] `.ps1` 脚本执行失败时输出结构化错误信息（行号、错误类型、调用栈）
- [ ] `hypo -r --prerelease beta` 能自动递增 prerelease 版本号
- [ ] 发版前 dry-run 能在沙箱中空跑脚本并报告潜在问题

---

## 六、跨阶段 Crate 选型总览

| Crate | 阶段一 | 阶段二 | 阶段三 | 阶段四 | 说明 |
|-------|:------:|:------:|:------:|:------:|------|
| `clap` | ✅ | + | + | + | CLI 框架 |
| `tokio` | ✅ | + | + | + | 异步运行时 |
| `reqwest` | ✅ | + | + | + | HTTP 客户端（registry 索引 + .hypo 下载 + GitHub API） |
| `sequoia-openpgp` | ✅ | + | + | + | 纯 Rust GPG 签名验证与生成（零外部依赖） |
| `serde` / `serde_json` / `toml` | ✅ | + | + | + | 序列化（registry 索引 + manifest） |
| `sha2` | ✅ | + | + | + | 哈希校验 |
| `semver` | ✅ | + | + | + | 版本解析 |
| `zip` | ✅ | + | + | + | .hypo 包打包与解包 |
| `petgraph` | ✅ | + | + | + | 依赖树构建与拓扑排序 |
| `anyhow` / `thiserror` | ✅ | + | + | + | 错误处理 |
| `tracing` | ✅ | + | + | + | 日志 |
| `indicatif` | ✅ | + | + | + | 下载进度条 |
| `directories` | ✅ | + | + | + | 目录定位 |
| `rusqlite` | ✅ | + | + | + | 本地数据库（含 `latest_seen_version` 降级防护） |
| `keyring` | | ✅ | + | + | PAT 加密存储（系统凭据存储抽象层） |
| `dialoguer` | ✅ | + | + | + | 交互提示 |
| `git2` | | | | ✅ | Seed List 多 remote 同步（阶段四） |
| `windows` | | ✅ | + | ✅ | Windows API（阶段二审计 ETW / 阶段四沙箱） |
| `landlock` | | | | ✅ | Linux 文件系统沙箱 |
| `libseccomp` | | | | ✅ | Linux 系统调用过滤 |
| `nix` | | | | ✅ | Linux namespaces |
| `fuzzy-matcher` | | | ✅ | + | 模糊搜索 |
| `sysinfo` | | | ✅ | + | 系统信息 |
| `self_update` 或自研 | | | ✅ | + | hypo 自更新模块 |

---

## 七、里程碑与依赖关系

```
阶段一 (MVP)
  └─ 核心信任链路 (registry → GPG → execute)
       │
       ├─ 阶段二 (安全加固)
       │    └─ freeze 闭环 + rollback + 初步沙箱
       │         │
       │         ├─ 阶段三 (生态完善)
       │         │    └─ 自动发版 + 镜像 + 跨平台补齐
       │         │         │
       │         │         └─ 阶段四 (高级特性)
       │         │              └─ 分布式同步 + 严格沙箱 + PS 深度支持
       │         │
       │         └─ (阶段三的沙箱增强依赖阶段二的沙箱基础)
       │
       └─ (阶段二的 freeze 命令依赖阶段一的 registry 读写能力)
```

**关键依赖约束**：
- 阶段二的 `hypo -r -f` 依赖阶段一的 registry 读写模块
- 阶段三的 `hypo -r` 完整发版依赖阶段二的 freeze 机制（发版时可标记旧版冻结）
- 阶段四的分布式同步依赖阶段三的镜像探测（Seed List 是镜像源的延伸）
- 阶段四的严格沙箱依赖阶段二的初步沙箱（同 trait，增强实现）

---

## 八、风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| GitHub API 速率限制 | 影响公钥发现与目录刷新 + 发版 + gh-pages 更新 | 本地缓存优先，API 仅刷新；支持 GITHUB_TOKEN 提升配额；gh-pages 更新走 Contents API 有速率限制，批量操作需分批 |
| .hypo 包下载源不可用 | 安装失败 | hypo-package.json 中可声明备用 URL；镜像源自动故障转移 |
| GitHub PAT 权限管理 | 发版失败或安全隐患 | 发版前校验 PAT scope（至少 `repo`）；PAT 通过 `keyring` crate 存入系统凭据存储（Windows DPAPI / macOS Keychain / Linux Secret Service），不落盘明文；支持 gh CLI 作为可选后端 |
| 降级攻击（gh-pages 被入侵） | 安装含已知漏洞的旧版本 | 本地 SQLite 记录 `latest_seen_version`，registry 返回低于本地记录值时 warn 并要求 `--force` |
| 公钥轮换过渡期被滥用 | 旧密钥泄露后攻击者持续签名恶意目录 | `key_rotation.old_key_retired_at` 时间戳，过期后拒绝旧密钥签名；过渡期固定 90 天 |
| 镜像可信度 | 安全风险 | 镜像内容仍需 GPG 验签，不信任未签名 registry |
| 包内文件哈希不一致 | 信任链断裂 | manifest `[hashes]` 表逐文件校验，任一不匹配拒绝执行 |
| 分片哈希表膨胀 | registry.json 体积过大 | 分片哈希按需加载；snapshot_version 增量同步；规模极大时可拆为多级分片 |
| 官方目录私钥泄露 | 根信任崩溃 | 双签名过渡期（新旧密钥同时有效）+ `key_update_url` 公钥轮换；分片哈希机制限制泄露影响范围 |
| 审计模块权限不足 | 阶段二审计能力受限 | 权限分级策略：管理员 ETW 全量，普通用户降级为 FileSystemWatcher + 环境变量注入 |
| 严格沙箱需管理员权限 | 阶段四沙箱难以普及 | 阶段二先做行为审计（无需管理员权限），阶段四沙箱作为 `--sandbox` 可选 flag，不强制启用 |
| GitHub username 改名 | 包名失效 | 改名后需重新注册官方目录分片；包名 `@owner/pkg` 中的 owner 不可变（建议开发者用组织名而非个人名） |
| search 全量遍历性能 | 开发者上千时 search 延迟高 | 全量遍历 + 并发请求 + 本地缓存（带 TTL）；规模上来后可引入 search-index.json 集中索引 |
| Contents API 非原子更新 | gh-pages 多文件更新中途失败导致不一致 | 失败回滚机制：先更新版本目录文件，最后更新 hypo-index.json（入口最后改） |
