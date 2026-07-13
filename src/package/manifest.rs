//! Manifest 数据结构（SPEC 7.5）。
//!
//! manifest.toml 同时存在于 gh-pages 版本目录和 .hypo 包内，
//! 包含包元数据、脚本声明、沙箱声明、依赖、文件哈希等。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 完整的 manifest.toml 内容。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// 包基本元数据。
    pub package: PackageMeta,

    /// 生命周期脚本声明。
    pub scripts: Scripts,

    /// 脚本解释器声明。
    #[serde(default)]
    pub interpreter: Interpreter,

    /// 沙箱安全声明。
    pub sandbox: SandboxDecl,

    /// 依赖声明。
    #[serde(default)]
    pub dependencies: Dependencies,

    /// 包内所有文件的 SHA256 哈希。
    pub hashes: HashMap<String, String>,
}

/// 包基本元数据 `[package]`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMeta {
    /// 包名。
    pub name: String,

    /// 版本号（SemVer）。
    pub version: String,

    /// 包描述。
    #[serde(default)]
    pub description: String,

    /// 作者（GitHub username）。
    pub author: String,

    /// GitHub 仓库 `owner/repo`。
    #[serde(default)]
    pub repo: String,

    /// 目标平台（windows / linux / macos / all）。
    pub platform: String,

    /// 目标架构列表。
    pub arch: Vec<String>,
}

/// 生命周期脚本声明 `[scripts]`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scripts {
    /// 安装脚本（必填）。
    pub install: String,

    /// 卸载脚本（可选）。
    #[serde(default)]
    pub uninstall: Option<String>,

    /// 更新脚本（可选）。
    #[serde(default)]
    pub update: Option<String>,

    /// 安装前钩子（可选）。
    #[serde(default)]
    pub pre_install: Option<String>,

    /// 安装后钩子（可选）。
    #[serde(default)]
    pub post_install: Option<String>,

    /// 卸载前钩子（可选）。
    #[serde(default)]
    pub pre_uninstall: Option<String>,

    /// 卸载后钩子（可选）。
    #[serde(default)]
    pub post_uninstall: Option<String>,

    /// 更新前钩子（可选）。
    #[serde(default)]
    pub pre_update: Option<String>,

    /// 更新后钩子（可选）。
    #[serde(default)]
    pub post_update: Option<String>,
}

/// 脚本解释器声明 `[interpreter]`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interpreter {
    /// 解释器类型（powershell / bash / zsh / python）。
    #[serde(rename = "type")]
    pub interpreter_type: String,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self {
            interpreter_type: "powershell".to_string(),
        }
    }
}

/// 沙箱安全声明 `[sandbox]`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxDecl {
    /// 脚本承诺的写入路径列表。
    #[serde(default)]
    pub allowed_write_paths: Vec<String>,

    /// 脚本承诺的网络出口域名列表。
    #[serde(default)]
    pub allowed_network_egress: Vec<String>,
}

/// 依赖声明 `[dependencies]`。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Dependencies {
    /// hypo 包依赖列表（如 `@bob/utils >= 2.0.0`）。
    #[serde(default)]
    pub hypo: Vec<String>,

    /// 系统包依赖列表。
    #[serde(default)]
    pub system: Vec<String>,
}

impl Manifest {
    /// 校验 manifest 完整性。
    ///
    /// 检查项：
    /// - install 脚本必填
    /// - sandbox 段必须存在
    /// - hashes 段必须非空
    pub fn validate(&self) -> Result<(), crate::error::HypoError> {
        if self.scripts.install.is_empty() {
            return Err(crate::error::HypoError::Config(
                "install 脚本路径为必填项".to_string(),
            ));
        }
        if self.hashes.is_empty() {
            return Err(crate::error::HypoError::Config(
                "[hashes] 段为空，必须包含包内所有文件的 SHA256".to_string(),
            ));
        }
        Ok(())
    }

    /// 从 TOML 字符串解析 Manifest。
    ///
    /// 内部调用 `toml::from_str`，将解析错误转换为 `HypoError`。
    pub fn parse(toml_str: &str) -> Result<Self, crate::error::HypoError> {
        toml::from_str(toml_str)
            .map_err(|e| crate::error::HypoError::Config(format!("manifest.toml 解析失败: {e}")))
    }

    /// 对比包内 manifest 与 gh-pages 上的 manifest 是否一致。
    ///
    /// 比较 [package] 段的 name / version / author / repo 字段，
    /// 以及 [hashes] 段是否完全相同。
    /// 任一不一致返回错误。
    pub fn compare_consistency(&self, remote: &Self) -> Result<(), crate::error::HypoError> {
        macro_rules! check {
            ($field:ident, $label:expr) => {
                if self.package.$field != remote.package.$field {
                    return Err(crate::error::HypoError::Config(format!(
                        "manifest 不一致：{} 不匹配（包内: {}, gh-pages: {}）",
                        $label, self.package.$field, remote.package.$field,
                    )));
                }
            };
        }

        check!(name, "包名");
        check!(version, "版本号");
        check!(author, "作者");
        check!(repo, "仓库");

        if self.hashes != remote.hashes {
            return Err(crate::error::HypoError::Config(
                "manifest 不一致：[hashes] 表不匹配".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_valid_manifest() -> Manifest {
        Manifest {
            package: PackageMeta {
                name: "my-tool".into(),
                version: "1.2.3".into(),
                description: "A cool CLI tool".into(),
                author: "alice".into(),
                repo: "alice/my-tool".into(),
                platform: "windows".into(),
                arch: vec!["x86_64".into(), "aarch64".into()],
            },
            scripts: Scripts {
                install: "tools/install.ps1".into(),
                uninstall: Some("tools/uninstall.ps1".into()),
                update: None,
                pre_install: None,
                post_install: None,
                pre_uninstall: None,
                post_uninstall: None,
                pre_update: None,
                post_update: None,
            },
            interpreter: Interpreter::default(),
            sandbox: SandboxDecl {
                allowed_write_paths: vec!["$env:LOCALAPPDATA/my-tool".into()],
                allowed_network_egress: vec!["github.com".into()],
            },
            dependencies: Dependencies::default(),
            hashes: {
                let mut h = HashMap::new();
                h.insert("tools/install.ps1".into(), "abc123...".into());
                h
            },
        }
    }

    #[test]
    fn test_manifest_validate_success() {
        let m = make_valid_manifest();
        assert!(m.validate().is_ok());
    }

    #[test]
    fn test_manifest_validate_missing_install() {
        let mut m = make_valid_manifest();
        m.scripts.install = String::new();
        assert!(m.validate().is_err());
    }

    #[test]
    fn test_manifest_validate_empty_hashes() {
        let mut m = make_valid_manifest();
        m.hashes.clear();
        assert!(m.validate().is_err());
    }

    #[test]
    fn test_manifest_toml_roundtrip() {
        let m = make_valid_manifest();
        let toml_str = toml::to_string_pretty(&m).expect("序列化失败");
        let restored: Manifest = toml::from_str(&toml_str).expect("反序列化失败");

        assert_eq!(restored.package.name, "my-tool");
        assert_eq!(restored.package.version, "1.2.3");
        assert_eq!(restored.scripts.install, "tools/install.ps1");
        assert_eq!(restored.interpreter.interpreter_type, "powershell");
        assert_eq!(restored.sandbox.allowed_write_paths.len(), 1);
        assert!(restored.hashes.contains_key("tools/install.ps1"));
        assert!(restored.validate().is_ok());
    }

    #[test]
    fn test_manifest_parses_all_hooks() {
        let m = make_valid_manifest();
        assert_eq!(m.scripts.uninstall.as_deref(), Some("tools/uninstall.ps1"));
        assert_eq!(m.scripts.update, None);
        assert_eq!(m.scripts.pre_install, None);
    }
}
