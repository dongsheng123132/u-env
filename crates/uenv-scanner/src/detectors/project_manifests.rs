// project.manifests detector — 项目清单解析：package.json / Cargo.toml / .nvmrc / .node-version / rust-toolchain.toml。
// layer=Project
// 产出：declared_toolchains (Map: "node" -> ">=22" 等)、package_manager。
// ⚠️ 包名里的私有 scope（如 @corp/*）必须脱敏。

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, Evidence, EvidenceKind, FactValue, Layer};

use crate::context::ScanContext;
use crate::detector::{Detector, DetectorMeta, DetectorResult};

pub struct ProjectManifests;

impl Detector for ProjectManifests {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "project.manifests",
            layer: Layer::Project,
            title: "项目清单",
            cost: Cost::Fast,
        }
    }

    fn applicable(&self, ctx: &ScanContext) -> bool {
        ctx.project_root.is_some()
    }

    fn detect(&self, ctx: &ScanContext) -> DetectorResult {
        let root = match &ctx.project_root {
            Some(p) => p.clone(),
            None => {
                return DetectorResult {
                    status: DetectStatus::Skipped,
                    summary: "未指定 --project，跳过".to_string(),
                    facts: BTreeMap::new(),
                    volatile: BTreeMap::new(),
                    evidence: vec![],
                };
            }
        };
        let root = std::path::absolute(&root).unwrap_or(root);

        let mut evidence = Vec::new();
        let mut declared: BTreeMap<String, String> = BTreeMap::new();
        let mut package_manager: Option<String> = None;
        // 是否有任何清单文件（决定 Ok vs Skipped）
        let mut has_manifest = false;

        // package.json
        let pkg_path = root.join("package.json");
        if pkg_path.is_file() {
            has_manifest = true;
            if let Ok(content) = std::fs::read_to_string(&pkg_path) {
                evidence.push(Evidence {
                    kind: EvidenceKind::File,
                    source: "package.json".to_string(),
                    exit_code: None,
                    excerpt: truncate_redacted(&content, ctx),
                });
                let (decl, pm) = parse_package_json(&content);
                declared.extend(decl);
                package_manager = pm;
            }
        }

        // Cargo.toml
        let cargo_path = root.join("Cargo.toml");
        if cargo_path.is_file() {
            has_manifest = true;
            if let Ok(content) = std::fs::read_to_string(&cargo_path) {
                evidence.push(Evidence {
                    kind: EvidenceKind::File,
                    source: "Cargo.toml".to_string(),
                    exit_code: None,
                    excerpt: truncate_redacted(&content, ctx),
                });
                if let Some(rs) = parse_cargo_toml(&content) {
                    declared.insert("rust".to_string(), rs);
                }
            }
        }

        // .nvmrc / .node-version
        for f in [".nvmrc", ".node-version"] {
            let p = root.join(f);
            if p.is_file() {
                if let Ok(content) = std::fs::read_to_string(&p) {
                    let v = content.trim();
                    if !v.is_empty() {
                        declared.insert("node".to_string(), v.to_string());
                    }
                    evidence.push(Evidence {
                        kind: EvidenceKind::File,
                        source: f.to_string(),
                        exit_code: None,
                        excerpt: v.to_string(),
                    });
                }
            }
        }

        // rust-toolchain.toml / rust-toolchain
        for f in ["rust-toolchain.toml", "rust-toolchain"] {
            let p = root.join(f);
            if p.is_file() {
                if let Ok(content) = std::fs::read_to_string(&p) {
                    // channel = "1.88" 或 "stable"
                    let mut ch: Option<String> = None;
                    for line in content.lines() {
                        let t = line.trim();
                        if let Some(rest) = t.strip_prefix("channel") {
                            if let Some(eq) = rest.find('=') {
                                let v = rest[eq + 1..].trim().trim_matches('"');
                                if !v.is_empty() {
                                    ch = Some(v.to_string());
                                    break;
                                }
                            }
                        }
                    }
                    if let Some(c) = ch {
                        declared.insert("rust".to_string(), c);
                    }
                    evidence.push(Evidence {
                        kind: EvidenceKind::File,
                        source: f.to_string(),
                        exit_code: None,
                        excerpt: truncate(&content, 500),
                    });
                }
            }
        }

        let mut facts = BTreeMap::new();
        if !declared.is_empty() {
            let m: BTreeMap<String, FactValue> = declared
                .into_iter()
                .map(|(k, v)| (k, FactValue::Str(v)))
                .collect();
            facts.insert("declared_toolchains".to_string(), FactValue::Map(m));
        }
        if let Some(pm) = package_manager {
            facts.insert("package_manager".to_string(), FactValue::Str(pm));
        }

        // 没有任何清单文件 → Skipped（不是项目）；有清单但无声明 → Ok（facts 可能空）
        if !has_manifest {
            return DetectorResult {
                status: DetectStatus::Skipped,
                summary: "未发现 package.json / Cargo.toml / .nvmrc 等清单".to_string(),
                facts,
                volatile: BTreeMap::new(),
                evidence,
            };
        }

        DetectorResult {
            status: DetectStatus::Ok,
            summary: format!("{} 个声明", facts.len()),
            facts,
            volatile: BTreeMap::new(),
            evidence,
        }
    }
}

/// 解析 package.json —— 与 IO 分离，独立可测。
/// 返回 (declared_toolchains, package_manager)
pub fn parse_package_json(content: &str) -> (BTreeMap<String, String>, Option<String>) {
    let mut declared: BTreeMap<String, String> = BTreeMap::new();
    let mut pm: Option<String> = None;

    let v: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(_) => return (declared, pm),
    };

    // engines: {"node": ">=22", "npm": ">=10"}
    if let Some(engines) = v.get("engines").and_then(|e| e.as_object()) {
        for (k, val) in engines {
            if let Some(s) = val.as_str() {
                if !s.is_empty() {
                    declared.insert(k.clone(), s.to_string());
                }
            }
        }
    }

    // packageManager: "pnpm@9.12.0"
    if let Some(p) = v.get("packageManager").and_then(|p| p.as_str()) {
        if !p.is_empty() {
            let name = p.split('@').next().unwrap_or(p).to_string();
            pm = Some(name);
        }
    }

    (declared, pm)
}

/// 解析 Cargo.toml 的 rust-version —— 与 IO 分离，独立可测。
/// 同时支持 [package] 与 [workspace.package] 段（workspace 根 Cargo.toml 常见）。
pub fn parse_cargo_toml(content: &str) -> Option<String> {
    let mut in_target = false;
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            let section = t.trim_start_matches('[').trim_end_matches(']').trim();
            in_target = section == "package" || section == "workspace.package";
            continue;
        }
        if in_target {
            if let Some(rest) = t.strip_prefix("rust-version") {
                if let Some(eq) = rest.find('=') {
                    let v = rest[eq + 1..].trim().trim_matches('"').to_string();
                    if !v.is_empty() {
                        return Some(v);
                    }
                }
            }
        }
    }
    None
}

fn truncate_redacted(content: &str, ctx: &ScanContext) -> String {
    let s = truncate(content, 2000);
    if ctx.redact { ctx.redact(&s) } else { s }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        s[..max].to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_package_engines_and_pm() {
        let content = r#"{
            "name": "my-app",
            "engines": {"node": ">=22", "npm": ">=10"},
            "packageManager": "pnpm@9.12.0"
        }"#;
        let (declared, pm) = parse_package_json(content);
        assert_eq!(declared.get("node").unwrap(), ">=22");
        assert_eq!(declared.get("npm").unwrap(), ">=10");
        assert_eq!(pm.as_deref(), Some("pnpm"));
    }

    #[test]
    fn parse_package_garbage() {
        let (declared, pm) = parse_package_json("not json");
        assert!(declared.is_empty());
        assert!(pm.is_none());
    }

    #[test]
    fn parse_cargo_rust_version() {
        let content = "[package]\nname = \"x\"\nrust-version = \"1.88\"\n\n[dependencies]\n";
        assert_eq!(parse_cargo_toml(content).as_deref(), Some("1.88"));
    }

    #[test]
    fn parse_cargo_no_rust_version() {
        assert_eq!(parse_cargo_toml("[package]\nname = \"x\"\n"), None);
        assert_eq!(parse_cargo_toml("garbage"), None);
    }
}
