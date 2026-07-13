//! 脚本执行器 trait。
//!
//! 定义跨平台脚本执行接口，MVP 实现 Windows PowerShell，
//! Linux/macOS 在阶段三补齐。

use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::process::ExitStatus;

/// 脚本执行器 trait。
///
/// 根据 manifest 声明的解释器类型选择对应实现：
/// - `powershell` → [`PowerShellExecutor`]（MVP）
/// - `bash` / `zsh` / `python` → 阶段三实现
pub trait ScriptExecutor {
    /// 执行指定脚本并返回退出状态。
    ///
    /// # 参数
    /// - `script_path`：脚本文件的绝对路径
    /// - `env_vars`：注入到脚本执行环境的环境变量
    fn execute(
        &self,
        script_path: &Path,
        env_vars: HashMap<String, String>,
    ) -> impl Future<Output = Result<ExitStatus, crate::error::HypoError>> + Send;
}
