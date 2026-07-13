//! Crypto 模块：GPG 签名验证（基于 sequoia-openpgp）。
//!
//! 纯 Rust 实现，Windows 上无需安装 Gpg4win。

pub mod github;
pub mod keyring;
pub mod verify;
