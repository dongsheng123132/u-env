// toolchain.node detector — 检测所有 PATH 中的 Node.js 安装。
// layer=Toolchain。
// PATH 原文放 volatile，解析出的可执行路径+版本放 facts (Set)。

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, EvidenceKind, FactValue, Layer};

use crate::context::{ScanContext, evidence_from_command};
use crate::detector::{Detector, DetectorMeta, DetectorResult};

pub struct ToolchainNode;

impl Detector for ToolchainNode {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "toolchain.node",
            layer: Layer::Toolchain,
            title: "Node.js",
            cost: Cost::Slow, // 每个命中都要跑 --version
        }
    }

    fn detect(&self, ctx: &ScanContext) -> DetectorResult {
        let hits = ctx.which_all("node");

        if hits.is_empty() {
            return DetectorResult {
                status: DetectStatus::Absent,
                summary: "Node.js not found in PATH".to_string(),
                facts: BTreeMap::new(),
                volatile: BTreeMap::new(),
                evidence: vec![],
            };
        }

        // PATH 原文放进 volatile
        let path_raw = std::env::var("PATH").unwrap_or_default();
        let mut volatile = BTreeMap::new();
        volatile.insert(
            "path_raw".to_string(),
            FactValue::Str(if ctx.redact {
                ctx.redact(&path_raw)
            } else {
                path_raw
            }),
        );

        // 用 ctx.run 执行 node --version（使用 PATH 中的第一个 node）
        let mut evidence = Vec::new();
        let outcome = ctx.run("node", &["--version"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "node --version",
            &outcome,
        ));

        let mut version_map: BTreeMap<String, FactValue> = BTreeMap::new();
        let found_versions = if outcome.ran && outcome.exit_code == Some(0) {
            let version = outcome.stdout.trim().to_string();
            let ver = version.strip_prefix('v').unwrap_or(&version);
            version_map.insert("primary".to_string(), FactValue::Version(ver.to_string()));
            1
        } else {
            0
        };

        // facts: 解析出的可执行路径集合（已脱敏）+ 版本
        let executable_paths: Vec<FactValue> = hits
            .iter()
            .map(|h| FactValue::Path(h.to_string_lossy().to_string()))
            .collect();

        let mut facts = BTreeMap::new();
        facts.insert("executables".to_string(), FactValue::Set(executable_paths));
        facts.insert("versions".to_string(), FactValue::Map(version_map));

        let status = if found_versions > 0 {
            DetectStatus::Ok
        } else {
            DetectStatus::Error
        };

        let summary = match hits.len() {
            0 => "Node.js not found".to_string(),
            n => {
                let ver_text = facts
                    .get("versions")
                    .and_then(|v| {
                        if let FactValue::Map(m) = v {
                            m.get("primary").cloned()
                        } else {
                            None
                        }
                    })
                    .map(|fv| format!("{fv:?}"))
                    .unwrap_or_else(|| "unknown".to_string());
                format!("Node.js {n} installation(s), version {ver_text}")
            }
        };

        DetectorResult {
            status,
            summary,
            facts,
            volatile,
            evidence,
        }
    }
}

/// 解析逻辑与 IO 分离 —— 独立可测
#[cfg(test)]
pub fn parse_node_versions(
    hit_paths: &[&str],
    version_outputs: &[Option<&str>],
) -> (Vec<String>, BTreeMap<String, String>) {
    let paths: Vec<String> = hit_paths.iter().map(|p| p.to_string()).collect();
    let mut versions = BTreeMap::new();
    for (i, path) in hit_paths.iter().enumerate() {
        if let Some(Some(output)) = version_outputs.get(i) {
            let v = output.trim().strip_prefix('v').unwrap_or(output.trim());
            versions.insert(path.to_string(), v.to_string());
        }
    }
    (paths, versions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_node() {
        let (paths, versions) = parse_node_versions(
            &[r"C:\Program Files\nodejs\node.exe"],
            &[Some("v22.11.0\n")],
        );
        assert_eq!(paths.len(), 1);
        assert_eq!(
            versions.get(r"C:\Program Files\nodejs\node.exe").unwrap(),
            "22.11.0"
        );
    }

    #[test]
    fn parse_multiple_nodes() {
        let (paths, versions) = parse_node_versions(
            &[
                r"C:\Program Files\nodejs\node.exe",
                r"D:\tools\nvm\v20.10.0\node.exe",
            ],
            &[Some("v22.11.0\n"), Some("v20.10.0\n")],
        );
        assert_eq!(paths.len(), 2);
        assert_eq!(
            versions.get(r"C:\Program Files\nodejs\node.exe").unwrap(),
            "22.11.0"
        );
        assert_eq!(
            versions.get(r"D:\tools\nvm\v20.10.0\node.exe").unwrap(),
            "20.10.0"
        );
    }

    #[test]
    fn parse_failed_version() {
        let (paths, versions) = parse_node_versions(&[r"C:\corrupted\node.exe"], &[None]);
        assert_eq!(paths.len(), 1);
        assert!(versions.is_empty());
    }

    #[test]
    fn parse_empty() {
        let (paths, versions) = parse_node_versions(&[], &[]);
        assert!(paths.is_empty());
        assert!(versions.is_empty());
    }
}
