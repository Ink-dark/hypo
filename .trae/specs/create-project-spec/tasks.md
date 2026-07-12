# Tasks

- [x] Task 1: 创建 docs/SPEC.md 项目定位章节
  - [x] SubTask 1.1: 编写"项目定位"——明确 hypo 为通用软件安装/卸载/更新管理器，平替 winget/Scoop/Chocolatey，支持任意语言软件分发
  - [x] SubTask 1.2: 编写"与现有工具对比表"——对比 winget/Scoop/Chocolatey/cargo-install/hypo 的去中心化、签名、跨平台、语言无关维度

- [x] Task 2: 创建 docs/SPEC.md 编码标准章节
  - [x] SubTask 2.1: 编写"Rust 编码标准"——edition 2021、stable toolchain、clippy -D warnings、rustfmt 强制
  - [x] SubTask 2.2: 编写"严禁 unsafe 代码"条款——主代码库零 unsafe，所有 FFI 通过审核的第三方 crate 间接调用
  - [x] SubTask 2.3: 编写"错误处理规范"——anyhow 用于应用层、thiserror 用于库层、禁用 unwrap/expect（仅测试可用）
  - [x] SubTask 2.4: 编写"文档要求"——所有 pub 项必须有 /// doc comment、cargo doc 零警告
  - [x] SubTask 2.5: 编写"依赖审核标准"——新增依赖需在 tasks.md 登记说明用途、禁止引入含 unsafe 且未经审计的 crate

- [x] Task 3: 创建 docs/SPEC.md 架构规格章节
  - [x] SubTask 3.1: 编写"模块结构"——src/ 完整目录树 + 每个模块职责说明
  - [x] SubTask 3.2: 编写"信任链规格"——根信任 → 开发者信任 → 包信任三层模型 + 90 天公钥轮换过渡期
  - [x] SubTask 3.3: 编写"包格式规格"——.hypo ZIP 布局（manifest.toml + tools/ + content/）+ manifest schema
  - [x] SubTask 3.4: 编写"Registry 结构规格"——registry.json / shard JSON / hypo-index.json / hypo-package.json 各自的 JSON schema

- [x] Task 4: 创建 docs/SPEC.md 安全规格章节
  - [x] SubTask 4.1: 编写"GPG 双签机制"——.hypo 整体签名 + manifest 签名 + sequoia-openpgp 实现
  - [x] SubTask 4.2: 编写"哈希校验规格"——registry.json shard_hashes + manifest 内 per-file SHA256
  - [x] SubTask 4.3: 编写"降级防护规格"——latest_seen_version 记录 + --force 要求 + 合法降级路径（freeze 机制）
  - [x] SubTask 4.4: 编写"信任模型规格"——--from-url 混合模式（GitHub Pages 自动信任 / TOFU 手动确认）

- [x] Task 5: 创建 docs/SPEC.md CLI 规格章节
  - [x] SubTask 5.1: 编写"子命令集"——init/install/uninstall/list/info/registry/config/init-r 全部子命令的参数定义
  - [x] SubTask 5.2: 编写"退出码规范"——0/1/2/10/11/12/13/14/15 各退出码定义
  - [x] SubTask 5.3: 编写"全局参数"——--verbose/--quiet/--no-color/--config 等

- [x] Task 6: 创建 docs/SPEC.md 兼容性规格章节
  - [x] SubTask 6.1: 编写"平台支持"——MVP Windows 优先 + trait 抽象跨平台
  - [x] SubTask 6.2: 编写"脚本解释器支持"——powershell/bash/zsh/python 四种 + MVP 仅 powershell
  - [x] SubTask 6.3: 编写"SemVer 合规"——版本格式 + 约束操作符 + prerelease 标签

- [x] Task 7: 创建 docs/SPEC.md 数据结构规格章节
  - [x] SubTask 7.1: 编写 registry.json schema（含 snapshot_version/shards/key_rotation/shard_hashes/key_update_url 字段）
  - [x] SubTask 7.2: 编写开发者分片 JSON schema（含 developer/gpg_fingerprint/base_pkg_url 字段）
  - [x] SubTask 7.3: 编写 hypo-index.json schema（含 packages/versions/freeze/rollback_version 字段）
  - [x] SubTask 7.4: 编写 hypo-package.json schema（含 version/platform/download_url/sha256/size 字段）
  - [x] SubTask 7.5: 编写 manifest.toml schema（含 [package]/[scripts]/[dependencies]/[sandbox]/[interpreter] 段）

# Task Dependencies

- Task 1-6 互相独立，可并行
- Task 7 依赖 Task 3（架构规格中已定义结构名称，Task 7 细化 schema）
- 所有 Task 依赖 roadmap.md 已存在且为最终版（已满足，第四轮迭代后）
