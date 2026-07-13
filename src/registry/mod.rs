//! Registry 模块：两层 Registry 拉取与缓存。
//!
//! 第一层为官方目录（分片结构），第二层为开发者自有 registry。

pub mod cache;
pub mod client;
pub mod trust;
pub mod types;
