# hypo

**High-trust Repository Operator** — 去中心化、跨平台的通用包管理器。

hypo **不打包软件**——它只验证并执行开发者提供的安装脚本。安全性基于端到端 GPG 签名信任链，不依赖中心化服务器。

---

## 为什么选 hypo？

| 特性 | hypo | winget | Scoop | Chocolatey | npm |
|------|------|--------|-------|------------|-----|
| **去中心化** | ✅ 两层 Registry | ❌ 单一源 | ❌ 单一 bucket | ❌ 单一源 | ❌ 单一 registry |
| **GPG 信任链** | ✅ 5 层验证 | ❌ | ❌ | ❌ | ❌ (仅 npm 签名) |
| **跨平台** | ✅ Windows/Linux/macOS | ✅ Windows | ❌ Windows only | ❌ Windows only | ✅ |
| **官方源消失后可运行** | ✅ fork 依赖树即可 | ❌ | ❌ | ❌ | ❌ (left-pad 事件) |
| **零外部 GPG 依赖** | ✅ sequoia (纯 Rust) | N/A | N/A | N/A | ❌ 需系统 gpg |
| **冻结/回退机制** | ✅ freeze + rollback | ❌ | ❌ | ❌ | ❌ |
| **降级攻击防护** | ✅ latest_seen_version | ❌ | ❌ | ❌ | ❌ |

### 核心差异化：生存能力

即使官方目录仓库消失，用户 fork 一份依赖树即可继续安装已注册的包。这是除 Rust 生态的 cargo 外，主流包管理工具均无法做到的。

---

## 快速开始

### 安装 hypo

```powershell
# 从 GitHub Releases 下载最新二进制（即将推出）
# 当前从源码构建：
git clone https://github.com/Ink-dark/hypo.git
cd hypo
cargo build --release
```

### 基本使用

```powershell
# 初始化本地环境
hypo init

# 从官方目录安装包
hypo install @owner/package-name

# 安装指定版本
hypo install @owner/package-name@1.2.3

# 强制执行（跳过降级/冻结保护）
hypo install @owner/package-name --force

# 从自定义 URL 安装
hypo install --from-url https://alice.github.io/hypo-pkgs

# 列出已安装包
hypo list

# 查看包详情
hypo info @owner/package-name

# 卸载包
hypo uninstall @owner/package-name

# 管理自定义 registry
hypo registry add my-mirror https://mirror.example.com/hypo
hypo registry list
hypo registry remove my-mirror
hypo registry export registries.txt

# 管理配置
hypo config get log_level
hypo config set log_level debug
```

---

## 信任链架构

```
硬编码根密钥指纹（编译在 hypo 二进制中）
  → 验证 registry.sig → 信任 registry.json（官方目录）
    → 分片 SHA256 与 shard_hashes 对比 → 信任开发者分片
      → 从分片提取开发者 GPG 指纹 → 验证 .hypo.sig + manifest.toml.sig
        → 逐文件 SHA256 与 manifest [hashes] 对比 → 执行安装脚本
```

五层信任，每一层都可独立验证，不依赖任何中心服务器在线。

### 完整安装流程（23 步）

```
1.  解析包名 @owner/pkg
2.  GET registry.json
3.  GET registry.sig
4.  GPG 验签 registry.json（硬编码根密钥）
5.  GET {首字母}/{owner}.json 分片
6.  SHA256 校验分片 vs registry.json 中的 shard_hashes
7.  从分片提取开发者 GPG 指纹，建立信任
8.  GET hypo-index.json（开发者包索引）
9.  确定目标版本（latest_version 或版本约束匹配）
10. 降级防护：对比本地 latest_seen_version
11. Freeze 检查：三态处理（自动回退 / 拒绝 / --force）
12. GET hypo-package.json（版本下载信息）
13. 选择当前平台条目
14. 下载 .hypo 包（带进度条）
15. 下载 .hypo.sig 签名
16. GPG 验签 .hypo 整体签名
17. 解包 ZIP → 临时目录（含 ZIP slip 防护）
18. GET manifest.toml + manifest.toml.sig（从 gh-pages）
19. GPG 验签 manifest.toml
20. 对比包内 manifest 与 gh-pages manifest 一致性
21. 逐文件 SHA256 校验 vs manifest [hashes] 表
22. 显示安装确认对话框（sandbox 声明 + 包信息）
23. 执行安装脚本 → 写入数据库 → 生成 hypo.lock
```

---

## 两层 Registry 结构

### 第一层：官方目录（hypo-directory）

```
hypo-org.github.io/directory/
├── registry.json          # 顶层索引：分片列表 + SHA256 哈希表
├── registry.sig           # GPG 分离签名
├── registry.sig.old       # 旧密钥签名（90天过渡期）
├── a/alice.json           # 按首字母分片，避免单文件瓶颈
├── b/bob.json
└── ...
```

### 第二层：开发者包仓库（hypo-pkgs）

```
alice.github.io/hypo-pkgs/
├── hypo-index.json        # 所有包索引
├── gpg-key.asc            # GPG 公钥（well-known 路径）
├── my-tool/
│   └── 1.2.3/
│       ├── hypo-package.json   # 版本下载信息（多平台）
│       ├── manifest.toml        # 包清单（含 [hashes]）
│       └── manifest.toml.sig    # manifest 独立签名
└── packages/
    └── my-tool-1.2.3-windows.hypo  # 包本体（ZIP 格式）
```

---

## .hypo 包格式

`.hypo` 文件是 ZIP 压缩包（nupkg 风格布局）：

```
my-tool-1.2.3-windows.hypo (ZIP)
├── manifest.toml           # 包元数据 + [hashes] 逐文件 SHA256
├── tools/
│   ├── install.ps1         # 安装脚本（必填）
│   ├── uninstall.ps1       # 卸载脚本（可选）
│   ├── pre_install.ps1     # 安装前钩子（可选）
│   └── post_install.ps1    # 安装后钩子（可选）
└── content/
    ├── bin/                # 二进制文件
    ├── config/             # 配置文件
    └── resources/          # 其他资源
```

### manifest.toml 示例

```toml
[package]
name = "my-tool"
version = "1.2.3"
author = "alice"
platform = "windows"
arch = ["x86_64", "aarch64"]

[scripts]
install = "tools/install.ps1"
uninstall = "tools/uninstall.ps1"

[interpreter]
type = "powershell"

[sandbox]
allowed_write_paths = ["$env:LOCALAPPDATA/my-tool"]
allowed_network_egress = ["github.com"]

[dependencies]
hypo = ["@bob/utils >= 2.0.0"]
system = ["powershell >= 5.0"]

[hashes]
"tools/install.ps1" = "abc123..."
"content/bin/my-tool.exe" = "def456..."
```

---

## 安全特性

### GPG 签名验证（纯 Rust，零外部依赖）

使用 `sequoia-openpgp` 实现，Windows 上无需安装 Gpg4win：
- **registry.sig** — 验证官方目录未被篡改（硬编码根密钥指纹）
- **.hypo.sig** — 验证包整体签名
- **manifest.toml.sig** — 独立验证包清单（与 .hypo 解耦）

### 公钥获取优先级

1. 本地 keyring 缓存（`~/.hypo/keyring/{fingerprint}.asc`）
2. GitHub Pages → GitHub GPG Keys API（自动）
3. `{base_pkg_url}/gpg-key.asc` well-known 路径（自动）
4. TOFU（Trust On First Use）— 用户手动确认后缓存

### 降级攻击防护

本地 SQLite 记录每个包的 `latest_seen_version`。若 registry 返回的版本低于本地记录，打印警告并要求 `--force`。

### Freeze 三态

| 场景 | 行为 |
|------|------|
| 不指定版本，目标版本已冻结 | 自动回退到 `rollback_version` + warn |
| 指定版本，不加 `--force` | 拒绝安装，提示回退命令 |
| 指定版本，加 `--force` | warn 后继续安装 |

### 退出码

| 码 | 含义 |
|----|------|
| 0 | 成功 |
| 1 | 通用错误 |
| 10 | 签名验证失败 |
| 11 | 哈希不匹配 |
| 12 | 网络错误 |
| 13 | Registry / 包未找到 |
| 14 | Freeze 违规 |
| 15 | 降级检测 |

### `--from-url` 信任模型

| URL 类型 | 策略 |
|----------|------|
| `*.github.io` | 自动信任，公钥从 GitHub API 拉取 |
| 其他 URL | TOFU 模式，首次安装显示公钥指纹供用户确认 |

### 安全防护清单

- [x] ZIP slip 路径穿越防护
- [x] URL userinfo 注入防护
- [x] 分片 owner 名校验（仅字母数字与连字符）
- [x] manifest 路径穿越防护
- [x] HTTP 重定向限制（最多 3 次）
- [x] 包内 vs gh-pages manifest 一致性对比

---

## 项目结构

```
src/
├── main.rs              # clap CLI 入口
├── lib.rs               # 模块 re-export
├── error.rs             # HypoError 枚举 + 退出码映射
├── constants.rs         # 硬编码常量（可被环境变量覆盖）
├── paths.rs             # ~/.hypo/ 目录管理
├── config.rs            # config.toml 读写
├── prg_bar.rs           # 终端进度条（零依赖，Cargo 风格）
├── commands/            # CLI 子命令实现
│   ├── init.rs          # hypo init
│   ├── install.rs       # hypo install（核心编排，~340行）
│   ├── uninstall.rs     # hypo uninstall
│   ├── list.rs          # hypo list
│   ├── info.rs          # hypo info
│   ├── registry.rs      # hypo registry add/remove/list/export
│   └── config_cmd.rs    # hypo config get/set
├── registry/            # 两层 Registry 拉取与缓存
│   ├── types.rs         # Serde 结构体（含 round-trip 测试）
│   ├── client.rs        # HTTP 拉取 + 分片 SHA256 校验
│   ├── cache.rs         # ETag/snapshot_version 条件缓存
│   └── trust.rs         # --from-url 信任模型（GitHub Pages / TOFU）
├── crypto/              # GPG 签名验证
│   ├── verify.rs        # sequoia 分离签名验证 + 密钥轮换
│   ├── keyring.rs       # 本地 keyring 缓存
│   └── github.rs        # GitHub GPG Keys API
├── package/             # .hypo 包处理
│   ├── reader.rs        # 下载（带进度条）+ ZIP 解包
│   ├── manifest.rs      # manifest.toml 解析与校验
│   └── hash.rs          # SHA256 逐文件哈希校验
├── executor/            # 脚本执行
│   ├── executor_trait.rs  # ScriptExecutor trait
│   └── powershell.rs    # PowerShell 执行器（含 dialoguer 确认）
├── sandbox/             # 沙箱（MVP 空实现，阶段四严格沙箱）
│   └── sandbox_trait.rs # PlatformSandbox trait
├── db/                  # 本地 SQLite
│   ├── schema.rs        # 表结构：packages, registries, schema_version
│   └── operations.rs    # CRUD 操作（含 latest_seen_version）
└── deps/                # 依赖解析
    ├── resolver.rs      # SemVer + petgraph 拓扑排序 + 循环检测
    └── lockfile.rs      # hypo.lock 生成与解析

tools/
└── gen-demo-pkg/        # 假 .hypo 包生成器（多文件多目录，用于测试）
```

---

## 构建与开发

### 前置要求

- Rust 1.70+
- Windows PowerShell 5.0+（Windows 平台）

### 命令

```bash
cargo build                           # 构建
cargo build --features cng            # Windows 构建（CNG 加密后端）
cargo clippy -- -D warnings           # Lint 检查
cargo fmt -- --check                  # 格式化检查
cargo test                            # 运行全部测试
cargo test crypto                     # 运行单个模块测试
cargo run --bin gen-demo-pkg          # 生成演示用假 .hypo 包
```

### 运行演示

```powershell
# 生成假包（输出到 tools/gen-demo-pkg/out/）
cargo run --bin gen-demo-pkg

# 设置 registry URL（指向测试数据）
$env:HYPO_REGISTRY_URL = "https://raw.githubusercontent.com/hypo-dev/hypo-registry/master"

# 初始化
cargo run --bin hypo -- init

# 安装演示包
cargo run --bin hypo -- install @hypo-dev/demo-tool --yes

# 查看已安装
cargo run --bin hypo -- list

# 查看详情
cargo run --bin hypo -- info @hypo-dev/demo-tool

# 卸载
cargo run --bin hypo -- uninstall @hypo-dev/demo-tool
```

---

## 路线图

| 阶段 | 目标 | 状态 |
|------|------|------|
| **阶段一 MVP** | 核心信任链路：registry → GPG → 脚本执行 | ✅ 基本完成 |
| **阶段二 安全加固** | freeze 闭环、行为审计、hypo doctor、lockfile 签名 | 计划中 |
| **阶段三 生态完善** | 自动发版、镜像探测、跨平台补齐、自更新 | 计划中 |
| **阶段四 高级特性** | 分布式同步、严格沙箱、PowerShell 深度支持 | 计划中 |

详见 [docs/TODO.md](docs/TODO.md) 和 [roadmap.md](roadmap.md)。

---

## 核心依赖

| Crate | 用途 |
|-------|------|
| `clap` (derive) | CLI 参数解析 |
| `tokio` | 异步运行时 |
| `reqwest` (rustls-tls) | HTTP 客户端 |
| `sequoia-openpgp` | 纯 Rust GPG 签名验证（无需外部 GPG） |
| `serde` / `serde_json` / `toml` | 序列化与配置解析 |
| `sha2` | SHA256 哈希 |
| `semver` | 版本号解析与比较 |
| `zip` | .hypo 包解包 |
| `petgraph` | 依赖图拓扑排序 |
| `anyhow` + `thiserror` | 错误处理 |
| `tracing` | 结构化日志 |
| `directories` | 跨平台目录定位 |
| `rusqlite` (bundled) | 本地 SQLite 数据库 |
| `dialoguer` | 用户交互确认 |

---

## 安全编码原则

- `#![forbid(unsafe_code)]` — 所有 unsafe 操作通过经审核的第三方 crate 间接调用
- 生产代码禁止 `.unwrap()` 和 `.expect()`
- 库层使用 `thiserror::Error`，应用层使用 `anyhow::Result`
- 所有 `pub` 项必须有文档注释
- 新增依赖优先纯 Rust 实现（如 `sequoia-openpgp` 而非 `gpgme`，`rustls` 而非 `openssl`）

---

## 目录说明

| 路径 | 用途 |
|------|------|
| `~/.hypo/config.toml` | 全局配置 |
| `~/.hypo/hypo.db` | SQLite 数据库（已安装包 + registry 记录） |
| `~/.hypo/keyring/` | GPG 公钥缓存（按指纹索引） |
| `~/.hypo/cache/` | HTTP 条件缓存（ETag / snapshot_version） |
| `~/.hypo/tmp/` | .hypo 包下载与解包临时目录 |
| `~/.hypo/logs/` | 审计日志目录（阶段二启用） |

---

## 许可证

MIT License
