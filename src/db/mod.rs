//! DB 模块：本地 SQLite 数据库。
//!
//! 存储已安装包信息（含降级防护字段）与 registry 配置。

pub mod operations;
pub mod schema;
