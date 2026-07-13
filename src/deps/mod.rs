//! Deps 模块：依赖解析与 lockfile 管理。
//!
//! 使用 `petgraph` 进行拓扑排序与循环检测，
//! `semver` 进行版本约束匹配。

pub mod lockfile;
pub mod resolver;
