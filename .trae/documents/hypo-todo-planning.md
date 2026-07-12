# Plan: 产出 docs/TODO.md 开发计划文档

## Summary

基于 `d:\hypo\roadmap.md`（第四轮迭代后的最终版），产出 `docs/TODO.md` 开发计划文档。覆盖全部四个阶段，阶段一（MVP）细化到任务级（含具体文件路径、模块结构、实现步骤、依赖关系），阶段二至四按特性粒度展开。

## Current State Analysis

- 项目目录 `d:\hypo` 当前仅有 `README.md`（一行简介）和 `roadmap.md`（完整路线图，已迭代四轮）
- **无任何代码**：无 Cargo.toml、无 src/、无 .gitignore
- 需从零搭建 Rust 项目骨架
- `docs/` 目录不存在，需创建

## Proposed Changes

### 唯一交付物：`docs/TODO.md`

创建 `docs/TODO.md`，内容结构如下：

---

#### 文档结构

```
# hypo 开发 TODO

## 阶段一：MVP（详细任务规划）

### MVP 模块架构总览
  - 完整的 src/ 目录树（每个文件/模块的职责说明）

### MVP 实现步骤（按依赖顺序，每步含子任务）

  Step 0: 项目初始化与骨架
  Step 1: 核心数据结构与 Trait 定义
  Step 2: Crypto 模块（sequoia 签名验证 PoC）
  Step 3: Registry 拉取与缓存模块
  Step 4: Package 下载与解包模块
  Step 5: 脚本执行器（Windows PowerShell）
  Step 6: 本地 SQLite 数据库
  Step 7: 依赖解析与 Lockfile
  Step 8: CLI 命令串联（init/install/uninstall/list/info/registry/config）
  Step 9: 集成测试与验收

  每步包含：
  - 涉及文件列表
  - 具体子任务（checkbox）
  - 依赖的前置步骤
  - 验证方法

### MVP 验收标准清单
  - 从 roadmap.md 验收标准部分完整搬入

## 阶段二：安全加固（特性级规划）
  - 6 个关键任务的简要描述 + 验收标准引用

## 阶段三：生态完善（特性级规划）
  - 6 个关键任务的简要描述 + 验收标准引用

## 阶段四：高级特性（特性级规划）
  - 4 个关键任务的简要描述 + 验收标准引用

## 跨阶段依赖关系图
```

---

#### 阶段一细化方案

**模块架构**（基于 roadmap.md 的 `commands/ registry/ crypto/ package/ sandbox/ executor/` 分层，进一步细化）：

```
src/
├── main.rs                      # clap CLI 入口
├── lib.rs                       # 测试用 re-export
├── error.rs                     # thiserror 错误类型
├── constants.rs                 # 硬编码官方公钥指纹、URL 等
├── paths.rs                     # ~/.hypo/ 目录结构管理
├── config.rs                    # config.toml 读写
├── commands/
│   ├── mod.rs
│   ├── init.rs                  # hypo init
│   ├── install.rs               # hypo install（含 --from-url）
│   ├── uninstall.rs             # hypo uninstall
│   ├── list.rs                  # hypo list
│   ├── info.rs                  # hypo info
│   ├── registry.rs              # hypo registry add/remove/list/export
│   └── config_cmd.rs            # hypo config get/set
├── registry/
│   ├── mod.rs
│   ├── types.rs                 # RegistryJson, ShardJson, HypoIndex, HypoPackage 等 serde 结构
│   ├── client.rs                # HTTP 拉取 registry.json/shards/index/package-index
│   ├── cache.rs                 # ETag/snapshot_version 缓存
│   └── trust.rs                 # --from-url 信任模型（GitHub Pages / TOFU）
├── crypto/
│   ├── mod.rs
│   ├── verify.rs                # sequoia 签名验证（registry.sig / .hypo.sig / manifest.toml.sig）
│   ├── keyring.rs               # 本地 keyring 缓存（~/.hypo/keyring/）
│   └── github.rs                # GitHub GPG Keys API 公钥拉取
├── package/
│   ├── mod.rs
│   ├── reader.rs                # .hypo ZIP 解包 + PackageReader trait
│   ├── manifest.rs              # manifest.toml 解析与校验
│   └── hash.rs                  # SHA256 逐文件哈希校验
├── executor/
│   ├── mod.rs
│   ├── trait.rs                 # ScriptExecutor trait
│   └── powershell.rs            # Windows PowerShell 执行器
├── sandbox/
│   ├── mod.rs
│   └── trait.rs                 # PlatformSandbox trait（MVP 空实现）
├── db/
│   ├── mod.rs
│   ├── schema.rs                # SQLite 表结构定义
│   └── operations.rs            # CRUD 操作
└── deps/
    ├── mod.rs
    ├── resolver.rs              # 版本解析 + 循环检测（petgraph）
    └── lockfile.rs              # hypo.lock 生成与解析（在线模式）
```

**实现步骤详细分解**（9 步，按依赖顺序）：

- **Step 0**：cargo init + Cargo.toml 依赖 + 模块骨架 + .gitignore
- **Step 1**：error.rs + constants.rs + paths.rs + registry/types.rs + 所有 trait 定义
- **Step 2**：crypto/verify.rs + crypto/keyring.rs + crypto/github.rs（sequoia PoC，可独立验证）
- **Step 3**：registry/client.rs + registry/cache.rs + registry/trust.rs（依赖 Step 1+2）
- **Step 4**：package/reader.rs + package/manifest.rs + package/hash.rs（依赖 Step 1）
- **Step 5**：executor/powershell.rs + executor/trait.rs（依赖 Step 1）
- **Step 6**：db/schema.rs + db/operations.rs（依赖 Step 1）
- **Step 7**：deps/resolver.rs + deps/lockfile.rs（依赖 Step 1+3）
- **Step 8**：commands/* 全部子命令 + config.rs + main.rs 串联（依赖 Step 1-7）
- **Step 9**：端到端测试 + 验收标准逐项检查

---

#### 阶段二至四的规划粒度

- **阶段二**：6 个任务（Freeze 闭环 / Rollback / 脚本审计 / gh-pages 更新 / hypo doctor / lockfile 签名），每个任务 3-5 行描述 + 验收标准
- **阶段三**：6 个任务（自动发版 / 镜像探测 / 系统依赖检查 / 跨平台补齐 / search+info / 自更新），同上
- **阶段四**：4 个任务（分布式同步 / 严格沙箱 / PowerShell 深度支持 / 发版高级特性），同上

## Assumptions & Decisions

1. **Rust edition 2021**，工具链 stable
2. **模块结构**基于 roadmap.md 第一节的分层决策，进一步拆分到可独立编译的粒度
3. **实现顺序**遵循 roadmap.md 第七节的依赖关系图，但将 Step 1（数据结构）和 Step 2（crypto PoC）提前，因为它们是技术风险点
4. **Step 2 优先于 Step 3-4**：sequoia 的 API 学习曲线是最大技术风险，先做 PoC 验证可行性
5. **阶段二至四**不细化到文件级，因为架构可能随阶段一实现演进
6. **docs/TODO.md** 使用 checkbox 语法（`- [ ]`），便于后续在 IDE 中跟踪进度
7. **不创建其他文件**：本次仅产出 docs/TODO.md，不创建 Cargo.toml 或任何代码文件

## Verification Steps

1. 检查 `docs/TODO.md` 是否存在且内容完整
2. 检查阶段一是否有 9 个 Step，每个 Step 是否有子任务 checkbox
3. 检查模块架构树是否与 roadmap.md 的分层决策一致
4. 检查阶段二至四是否覆盖 roadmap.md 中的所有关键任务
5. 检查验收标准是否完整搬入
