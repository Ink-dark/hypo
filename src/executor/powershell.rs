//! Windows PowerShell 脚本执行器。
//!
//! MVP 阶段的默认执行器，通过 `tokio::process::Command`
//! 调用 `powershell.exe`，注入 hypo 环境变量并捕获执行结果。

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

use crate::error::HypoError;
use crate::executor::executor_trait::ScriptExecutor;

/// PowerShell 脚本执行器。
///
/// 持有包安装所需的上下文信息，在执行脚本前通过 [`dialoguer`] 获取用户确认。
#[derive(Clone)]
pub struct PowerShellExecutor {
    /// 包名（如 `@alice/my-tool`）。
    pub package_name: String,
    /// 版本号。
    pub package_version: String,
    /// 沙箱声明摘要（用于安装前展示）。
    pub sandbox_info: String,
    /// 安装目标目录。
    pub install_dir: PathBuf,
    /// content/ 资源目录路径。
    pub content_dir: PathBuf,
    /// 是否跳过安装前确认（`--yes` flag）。
    pub skip_confirm: bool,
}

impl PowerShellExecutor {
    /// 创建新的 PowerShell 执行器。
    pub fn new(
        package_name: &str,
        package_version: &str,
        install_dir: PathBuf,
        content_dir: PathBuf,
    ) -> Self {
        Self {
            package_name: package_name.to_string(),
            package_version: package_version.to_string(),
            sandbox_info: String::new(),
            install_dir,
            content_dir,
            skip_confirm: false,
        }
    }

    /// 设置沙箱声明。
    pub fn with_sandbox(mut self, info: &str) -> Self {
        self.sandbox_info = info.to_string();
        self
    }

    /// 显示安装前确认对话框，返回用户是否同意。
    fn confirm_install(&self) -> Result<bool, HypoError> {
        use dialoguer::Confirm;

        println!();
        println!("═══ hypo 安装确认 ═══");
        println!("  包名:    {}", self.package_name);
        println!("  版本:    {}", self.package_version);
        println!("  目标:    {}", self.install_dir.display());
        if !self.sandbox_info.is_empty() {
            println!("  沙箱声明: {}", self.sandbox_info);
        }
        println!("══════════════════════");
        println!();

        Confirm::new()
            .with_prompt("确认执行安装脚本？")
            .default(false)
            .interact()
            .map_err(|e| HypoError::PowerShell(format!("用户确认对话框失败: {e}")))
    }
}

impl ScriptExecutor for PowerShellExecutor {
    fn execute(
        &self,
        script_path: &Path,
        extra_env: HashMap<String, String>,
    ) -> impl Future<Output = Result<ExitStatus, HypoError>> + Send {
        let script = script_path.to_path_buf();
        let pkg_name = self.package_name.clone();
        let pkg_ver = self.package_version.clone();
        let install_dir = self.install_dir.clone();
        let content_dir = self.content_dir.clone();
        let skip = self.skip_confirm;
        let confirmed = if skip {
            None
        } else {
            Some(self.confirm_install())
        };

        async move {
            // 检查用户确认（非阻塞，在 async 块内统一处理）
            if let Some(result) = confirmed {
                match result {
                    Ok(false) => {
                        return Err(HypoError::PowerShell("用户取消了安装".to_string()));
                    }
                    Err(e) => return Err(e),
                    Ok(true) => {} // 继续执行
                }
            }

            let mut cmd = tokio::process::Command::new("powershell.exe");
            cmd.arg("-ExecutionPolicy")
                .arg("Bypass")
                .arg("-NoProfile")
                .arg("-NonInteractive")
                .arg("-File")
                .arg(&script);

            cmd.env("HYPO_PKG_NAME", &pkg_name);
            cmd.env("HYPO_PKG_VERSION", &pkg_ver);
            cmd.env("HYPO_INSTALL_DIR", &install_dir);
            cmd.env("HYPO_CONTENT_DIR", &content_dir);

            for (k, v) in extra_env {
                cmd.env(k, v);
            }

            cmd.stdout(std::process::Stdio::inherit());
            cmd.stderr(std::process::Stdio::inherit());

            let output = cmd
                .output()
                .await
                .map_err(|e| HypoError::PowerShell(format!("无法启动 PowerShell: {e}")))?;

            if output.status.success() {
                Ok(output.status)
            } else {
                let code = output.status.code().unwrap_or(-1);
                Err(HypoError::PowerShell(format!(
                    "PowerShell 脚本以退出码 {code} 结束"
                )))
            }
        }
    }
}

/// Bash 脚本执行器 stub（阶段三实现）。
#[derive(Clone)]
pub struct BashExecutor;

impl ScriptExecutor for BashExecutor {
    async fn execute(
        &self,
        _script_path: &Path,
        _env_vars: HashMap<String, String>,
    ) -> Result<ExitStatus, HypoError> {
        todo!("Bash 执行器将在阶段三实现")
    }
}

/// Zsh 脚本执行器 stub（阶段三实现）。
#[derive(Clone)]
pub struct ZshExecutor;

impl ScriptExecutor for ZshExecutor {
    async fn execute(
        &self,
        _script_path: &Path,
        _env_vars: HashMap<String, String>,
    ) -> Result<ExitStatus, HypoError> {
        todo!("Zsh 执行器将在阶段三实现")
    }
}

/// Python 脚本执行器 stub（阶段三实现）。
#[derive(Clone)]
pub struct PythonExecutor;

impl ScriptExecutor for PythonExecutor {
    async fn execute(
        &self,
        _script_path: &Path,
        _env_vars: HashMap<String, String>,
    ) -> Result<ExitStatus, HypoError> {
        todo!("Python 执行器将在阶段三实现")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_clone() {
        let exe = PowerShellExecutor::new(
            "test-pkg",
            "1.0.0",
            PathBuf::from("/tmp/install"),
            PathBuf::from("/tmp/content"),
        );
        let exe2 = exe.clone();
        assert_eq!(exe.package_name, exe2.package_name);
    }

    #[test]
    fn test_skip_confirm() {
        let mut exe = PowerShellExecutor::new(
            "test-pkg",
            "1.0.0",
            PathBuf::from("/tmp/install"),
            PathBuf::from("/tmp/content"),
        );
        exe.skip_confirm = true;
        assert!(exe.skip_confirm);
    }

    #[test]
    fn test_with_sandbox() {
        let exe = PowerShellExecutor::new(
            "test-pkg",
            "1.0.0",
            PathBuf::from("/tmp/install"),
            PathBuf::from("/tmp/content"),
        )
        .with_sandbox("allowed_write_paths: [~/test]");
        assert!(exe.sandbox_info.contains("allowed_write_paths"));
    }
}
