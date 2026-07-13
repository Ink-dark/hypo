//! hypo (High-trust Repository Operator) 核心库。
//!
//! 去中心化的通用软件包管理器，基于 GPG 信任链实现安全的
//! registry 拉取、包验证与脚本执行。

#![forbid(unsafe_code)]

/// CLI 子命令实现模块。
pub mod commands;
/// 全局配置读写模块。
pub mod config;
/// 硬编码常量（官方目录 URL、公钥指纹等）。
pub mod constants;
/// GPG 签名验证模块（sequoia-openpgp）。
pub mod crypto;
/// 本地 SQLite 数据库模块。
pub mod db;
/// 依赖解析与 lockfile 模块。
pub mod deps;
/// 自定义错误类型与退出码映射。
pub mod error;
/// 跨平台脚本执行器模块。
pub mod executor;
/// .hypo 包处理（解包、manifest、哈希校验）模块。
pub mod package;
/// `~/.hypo/` 目录结构管理模块。
pub mod paths;
/// 两层 Registry 拉取与缓存模块。
pub mod registry;
/// 沙箱隔离模块（MVP 空实现）。
pub mod sandbox;
