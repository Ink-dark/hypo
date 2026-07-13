//! 依赖解析器：版本约束匹配 + petgraph 拓扑排序 + 循环检测。
//!
//! 支持 `@owner/pkg >= 1.0.0` 格式的 hypo 依赖声明。

use std::collections::HashMap;

use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use semver::{Version, VersionReq};

use crate::error::HypoError;

/// 解析后的依赖声明。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDep {
    /// 包所有者。
    pub owner: String,
    /// 包名。
    pub name: String,
    /// 原始版本约束字符串（如 `>= 2.0.0`）。
    pub constraint_str: String,
}

/// 解析后的依赖树节点。
#[derive(Debug, Clone)]
pub struct ResolvedDep {
    /// 包所有者。
    pub owner: String,
    /// 包名。
    pub name: String,
    /// 选定的确切版本。
    pub version: String,
    /// 可用版本列表（已排序）。
    pub available_versions: Vec<String>,
}

/// 解析依赖字符串 `@owner/pkg [constraint]`。
///
/// # 示例
/// - `@alice/my-tool >= 1.0.0` → `ParsedDep { owner: "alice", name: "my-tool", constraint_str: ">= 1.0.0" }`
/// - `@bob/utils` → `constraint_str = ""`（任意版本）
pub fn parse_dep_string(input: &str) -> Result<ParsedDep, HypoError> {
    let input = input.trim();
    let without_at = input
        .strip_prefix('@')
        .ok_or_else(|| HypoError::Config(format!("无效的依赖声明（缺少 @ 前缀）: {input}")))?;

    // 分离 `owner/name` 与约束部分
    // 约束由空格分隔，后跟 > < = ^ ~ 之一
    let (pkg_part, constraint_part) = {
        let bytes = without_at.as_bytes();
        let mut split_pos = None;
        for i in 0..bytes.len() {
            if bytes[i] == b' ' && i + 1 < bytes.len() {
                let next = bytes[i + 1];
                if matches!(next, b'>' | b'<' | b'=' | b'^' | b'~') {
                    split_pos = Some(i);
                    break;
                }
            }
        }
        match split_pos {
            Some(pos) => {
                let (a, b) = without_at.split_at(pos);
                (a.trim(), b.trim().to_string())
            }
            None => (without_at.trim(), String::new()),
        }
    };

    let (owner, name) = pkg_part
        .split_once('/')
        .ok_or_else(|| HypoError::Config(format!("无效的依赖声明（缺少 / 分隔符）: {input}")))?;

    Ok(ParsedDep {
        owner: owner.to_string(),
        name: name.to_string(),
        constraint_str: constraint_part,
    })
}

/// 检查版本是否满足约束。
///
/// `constraint_str` 为空时匹配任意版本。
pub fn version_matches(constraint_str: &str, version: &str) -> bool {
    if constraint_str.is_empty() {
        return true;
    }

    match Version::parse(version) {
        Ok(ver) => match VersionReq::parse(constraint_str) {
            Ok(req) => req.matches(&ver),
            Err(_) => false,
        },
        Err(_) => false,
    }
}

/// 从版本列表中选择满足约束的最高版本。
///
/// `versions` 应为已排序的版本列表（最新在前），
/// `constraint_str` 为空时返回第一个（最新）版本。
pub fn select_best_version<'a>(
    constraint_str: &str,
    versions: &'a [String],
) -> Result<&'a str, HypoError> {
    // 按 SemVer 降序排列
    let mut parsed: Vec<(&str, Version)> = versions
        .iter()
        .filter_map(|v| Version::parse(v).ok().map(|pv| (v.as_str(), pv)))
        .collect();
    parsed.sort_by(|a, b| b.1.cmp(&a.1));

    if constraint_str.is_empty() {
        return parsed
            .first()
            .map(|(s, _)| *s)
            .ok_or_else(|| HypoError::Config("无可用的版本".to_string()));
    }

    let req = VersionReq::parse(constraint_str)
        .map_err(|e| HypoError::Config(format!("无效的版本约束 '{constraint_str}': {e}")))?;

    for (ver_str, ver) in &parsed {
        if req.matches(ver) {
            return Ok(ver_str);
        }
    }

    Err(HypoError::Config(format!(
        "找不到满足约束 '{constraint_str}' 的版本"
    )))
}

/// 使用 petgraph 对依赖图进行拓扑排序。
///
/// 若存在循环依赖，返回错误并包含涉及的节点信息。
pub fn topological_sort(
    deps: &[ResolvedDep],
    edges: &[(usize, usize)], // (from_index, to_index) in deps array
) -> Result<Vec<usize>, HypoError> {
    let mut graph = DiGraph::<usize, ()>::new();
    let mut node_map: HashMap<usize, NodeIndex> = HashMap::new();

    for (i, _dep) in deps.iter().enumerate() {
        let node = graph.add_node(i);
        node_map.insert(i, node);
    }

    for (from, to) in edges {
        if let (Some(&from_node), Some(&to_node)) = (node_map.get(from), node_map.get(to)) {
            graph.add_edge(from_node, to_node, ());
        }
    }

    match toposort(&graph, None) {
        Ok(order) => Ok(order.iter().map(|n| graph[*n]).collect()),
        Err(cycle_err) => {
            let node_idx = cycle_err.node_id();
            let dep_index = graph[node_idx];
            let dep = &deps[dep_index];
            Err(HypoError::Config(format!(
                "检测到循环依赖，涉及：@{} / {}",
                dep.owner, dep.name
            )))
        }
    }
}

/// 递归解析依赖树。
///
/// MVP 简化版：仅做版本约束匹配与排序，不进行实际的递归 registry 拉取。
/// 实际递归拉取将在 Step 8 与 CLI 集成时通过 `HypoIndex` 数据完成。
pub fn resolve(
    root_owner: &str,
    root_name: &str,
    version_constraint: &str,
    available_versions: &[String], // 按版本降序排列
) -> Result<Vec<ResolvedDep>, HypoError> {
    let best = select_best_version(version_constraint, available_versions)?;

    let root = ResolvedDep {
        owner: root_owner.to_string(),
        name: root_name.to_string(),
        version: best.to_string(),
        available_versions: available_versions.to_vec(),
    };

    Ok(vec![root])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dep_with_constraint() {
        let dep = parse_dep_string("@alice/my-tool >= 1.0.0").unwrap();
        assert_eq!(dep.owner, "alice");
        assert_eq!(dep.name, "my-tool");
        assert_eq!(dep.constraint_str, ">= 1.0.0");
    }

    #[test]
    fn test_parse_dep_no_constraint() {
        let dep = parse_dep_string("@bob/utils").unwrap();
        assert_eq!(dep.owner, "bob");
        assert_eq!(dep.name, "utils");
        assert!(dep.constraint_str.is_empty());
    }

    #[test]
    fn test_parse_dep_caret() {
        let dep = parse_dep_string("@org/pkg ^1.2.3").unwrap();
        assert_eq!(dep.constraint_str, "^1.2.3");
    }

    #[test]
    fn test_parse_dep_invalid() {
        assert!(parse_dep_string("no-at-sign").is_err());
        assert!(parse_dep_string("@no-slash").is_err());
    }

    #[test]
    fn test_version_matches() {
        assert!(version_matches(">= 1.0.0", "1.5.0"));
        assert!(version_matches(">= 1.0.0", "2.0.0"));
        assert!(!version_matches(">= 2.0.0", "1.5.0"));
        assert!(version_matches("", "any-version"));
        assert!(version_matches("^1.2.3", "1.5.0"));
        assert!(!version_matches("^1.2.3", "2.0.0"));
        assert!(version_matches("~1.2.3", "1.2.5"));
        assert!(!version_matches("~1.2.3", "1.3.0"));
    }

    #[test]
    fn test_select_best_version() {
        let versions: Vec<String> = ["2.0.0", "1.5.0", "1.0.0", "0.9.0"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        assert_eq!(select_best_version(">= 1.0.0", &versions).unwrap(), "2.0.0");
        assert_eq!(
            select_best_version(">= 1.0.0, < 2.0.0", &versions).unwrap(),
            "1.5.0"
        );
        assert_eq!(select_best_version("", &versions).unwrap(), "2.0.0");
        assert!(select_best_version(">= 3.0.0", &versions).is_err());
    }

    #[test]
    fn test_topological_sort_no_cycle() {
        let deps = vec![
            ResolvedDep {
                owner: "root".into(),
                name: "app".into(),
                version: "1.0.0".into(),
                available_versions: vec![],
            },
            ResolvedDep {
                owner: "dep".into(),
                name: "lib".into(),
                version: "2.0.0".into(),
                available_versions: vec![],
            },
        ];
        let edges = vec![(0, 1)]; // root → dep
        let order = topological_sort(&deps, &edges).unwrap();
        // root 应在 dep 之前
        assert_eq!(order[0], 0);
        assert_eq!(order[1], 1);
    }

    #[test]
    fn test_topological_sort_cycle_detected() {
        let deps = vec![
            ResolvedDep {
                owner: "a".into(),
                name: "pkg-a".into(),
                version: "1.0.0".into(),
                available_versions: vec![],
            },
            ResolvedDep {
                owner: "b".into(),
                name: "pkg-b".into(),
                version: "1.0.0".into(),
                available_versions: vec![],
            },
        ];
        let edges = vec![(0, 1), (1, 0)]; // a → b → a
        assert!(topological_sort(&deps, &edges).is_err());
    }
}
