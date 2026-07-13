//! 沙箱隔离 trait。
//!
//! MVP 阶段为空实现（所有方法返回 Ok），
//! 阶段二实现行为审计（事后告警），阶段四实现严格沙箱（事前拦截）。

/// 平台沙箱抽象 trait。
pub trait PlatformSandbox {
    /// 检查当前平台是否支持沙箱能力。
    fn is_available(&self) -> bool;

    /// 审计脚本的写入操作。
    ///
    /// MVP 空实现始终返回 `Ok(())`。
    /// 阶段二记录写入路径到审计日志，超出 `allowed_write_paths` 范围时事后告警。
    /// 阶段四配合 `--sandbox` flag 做事前拦截。
    fn audit_write(&self, path: &str) -> Result<(), crate::error::HypoError>;
}
