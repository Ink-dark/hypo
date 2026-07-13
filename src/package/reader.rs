//! .hypo 包读取 trait。
//!
//! 定义解包、读取 manifest、列出文件的标准接口，
//! 具体实现在 Step 4 中完成。

use std::path::Path;

use crate::package::manifest::Manifest;

/// .hypo 包读取器 trait。
///
/// 封装 .hypo（ZIP 压缩包）的解包与元数据读取操作。
pub trait PackageReader {
    /// 从包中读取 manifest.toml，反序列化为 [`Manifest`] 结构体。
    fn read_manifest(&self) -> Result<Manifest, crate::error::HypoError>;

    /// 将包内容解压到目标目录。
    fn extract_to(&self, dest: &Path) -> Result<(), crate::error::HypoError>;

    /// 列出包内所有文件路径。
    fn list_files(&self) -> Vec<String>;
}
