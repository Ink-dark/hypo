use thiserror::Error;

/// hypo 自定义错误类型。
///
/// 每个变体映射到 SPEC 5.2 定义的退出码，通过 [`exit_code`](HypoError::exit_code) 方法获取。
#[derive(Debug, Error)]
pub enum HypoError {
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("SQLite 错误: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Sequoia/OpenPGP 错误: {0}")]
    Sequoia(String),

    #[error("PowerShell 执行错误: {0}")]
    PowerShell(String),

    #[error("网络错误: {0}")]
    Network(String),

    #[error("签名验证失败: {0}")]
    SignatureVerification(String),

    #[error("哈希不匹配: {0}")]
    HashMismatch(String),

    #[error("Registry 未找到: {0}")]
    RegistryNotFound(String),

    #[error("包未找到: {0}")]
    PackageNotFound(String),

    #[error("GPG 公钥未找到，指纹: {0}")]
    GpgNoPubkey(String),

    #[error("Freeze 违规: {0}")]
    FreezeViolation(String),

    #[error("降级检测: {0}")]
    DowngradeDetected(String),

    #[error("数据库错误: {0}")]
    Database(String),

    #[error("配置错误: {0}")]
    Config(String),
}

impl HypoError {
    /// 返回 SPEC 5.2 定义的退出码。
    ///
    /// | 退出码 | 含义 |
    /// |--------|------|
    /// | 0 | 成功 |
    /// | 1 | 通用错误 |
    /// | 10 | 签名验证失败 |
    /// | 11 | 哈希不匹配 |
    /// | 12 | 网络错误 |
    /// | 13 | Registry / 包未找到 |
    /// | 14 | Freeze 违规 |
    /// | 15 | 降级检测 |
    pub fn exit_code(&self) -> i32 {
        match self {
            HypoError::SignatureVerification(_) => 10,
            HypoError::HashMismatch(_) => 11,
            HypoError::Network(_) => 12,
            HypoError::RegistryNotFound(_) | HypoError::PackageNotFound(_) => 13,
            HypoError::FreezeViolation(_) => 14,
            HypoError::DowngradeDetected(_) => 15,
            // 所有其他错误映射为通用错误（退出码 1）
            _ => 1,
        }
    }
}
