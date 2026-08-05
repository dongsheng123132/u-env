// toolchain.rust detector — rustup 状态、toolchain 列表、active、host triple、targets、cargo/rustc 路径。
// layer=Toolchain
// 数据源：rustup show、rustup target list --installed、which_all("cargo")

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, EvidenceKind, FactValue, Layer};

use crate::context::{ScanContext, evidence_from_command};
use crate::detector::{Detector, DetectorMeta, DetectorResult};

pub struct ToolchainRust;

impl Detector for ToolchainRust {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "toolchain.rust",
            layer: Layer::Toolchain,
            title: "Rust 工具链",
            cost: Cost::Slow,
        }
    }

    fn detect(&self, ctx: &ScanContext) -> DetectorResult {
        let mut evidence = Vec::new();

        let show = ctx.run("rustup", &["show"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "rustup show",
            &show,
        ));

        // 未安装 → Absent
        if !show.ran {
            return DetectorResult {
                status: DetectStatus::Absent,
                summary: "rustup 未安装（rustup 不在 PATH）".to_string(),
                facts: BTreeMap::new(),
                volatile: BTreeMap::new(),
                evidence,
            };
        }

        let targets = ctx.run("rustup", &["target", "list", "--installed"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "rustup target list --installed",
            &targets,
        ));

        let cargo_hits = ctx.which_all("cargo");
        let cargo_paths: Vec<FactValue> = cargo_hits
            .iter()
            .map(|p| FactValue::Path(p.to_string_lossy().to_string()))
            .collect();

        let mut facts = parse_rustup(&show.stdout, &targets.stdout);
        if !cargo_paths.is_empty() {
            facts.insert("cargo_paths".to_string(), FactValue::Set(cargo_paths));
        }

        let (status, summary) = if facts.is_empty() {
            (DetectStatus::Error, "rustup show 输出无法解析".to_string())
        } else {
            let active = facts
                .get("active_toolchain")
                .map(|v| match v {
                    FactValue::Str(s) => s.clone(),
                    _ => String::new(),
                })
                .unwrap_or_default();
            (
                DetectStatus::Ok,
                if active.is_empty() {
                    "rustup 已安装".to_string()
                } else {
                    format!("active {active}")
                },
            )
        };

        DetectorResult {
            status,
            summary,
            facts,
            volatile: BTreeMap::new(),
            evidence,
        }
    }
}

/// 解析 rustup show 输出 —— 与 IO 分离，独立可测。
/// 输出分块：Default host / installed toolchains / active toolchain (name + targets)
pub fn parse_rustup(show_out: &str, targets_out: &str) -> BTreeMap<String, FactValue> {
    let mut facts = BTreeMap::new();
    let mut in_toolchains = false;
    let mut in_active = false;
    let mut toolchains: Vec<FactValue> = Vec::new();

    for line in show_out.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("Default host") {
            if let Some(v) = extract_value(line, "Default host") {
                facts.insert("host_triple".to_string(), FactValue::Str(v));
            }
            continue;
        }
        if line.starts_with("rustup home") {
            continue;
        }
        if line.starts_with("installed toolchains") || line.starts_with("Installed toolchains") {
            in_toolchains = true;
            in_active = false;
            continue;
        }
        if line.starts_with("active toolchain") || line.starts_with("Active toolchain") {
            in_toolchains = false;
            in_active = true;
            continue;
        }
        // 分隔线
        if line.chars().all(|c| c == '-') {
            continue;
        }
        if in_toolchains {
            // "stable-x86_64-pc-windows-msvc (active, default)" / "1.95-x86_64-pc-windows-msvc"
            let name = line.split_whitespace().next().unwrap_or("").to_string();
            if !name.is_empty() {
                toolchains.push(FactValue::Str(name));
            }
        } else if in_active {
            if let Some(rest) = line.strip_prefix("name:") {
                let v = rest.trim().to_string();
                if !v.is_empty() {
                    facts.insert("active_toolchain".to_string(), FactValue::Str(v));
                }
            } else if line.starts_with("installed targets") || line.starts_with("Installed targets")
            {
                // targets 在下一行（缩进），rustup target list 已有全量，这里跳过
            }
        }
    }

    if !toolchains.is_empty() {
        facts.insert("toolchains".to_string(), FactValue::Set(toolchains));
    }

    // installed targets（来自 rustup target list --installed）
    let mut targets: Vec<FactValue> = Vec::new();
    for line in targets_out.lines() {
        let t = line.trim();
        if !t.is_empty() {
            targets.push(FactValue::Str(t.to_string()));
        }
    }
    if !targets.is_empty() {
        facts.insert("targets".to_string(), FactValue::Set(targets));
    }

    facts
}

/// "Default host: x86_64-pc-windows-msvc" → 冒号后内容（兼容中英冒号）
fn extract_value(line: &str, key: &str) -> Option<String> {
    for sep in [":", "："] {
        if let Some(idx) = line.find(sep) {
            let before = line[..idx].trim();
            if before.eq_ignore_ascii_case(key) {
                let v = line[idx + sep.len()..].trim();
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_rustup_show() {
        let show = "Default host: x86_64-pc-windows-msvc\n\
                    rustup home:  C:\\Users\\me\\.rustup\n\n\
                    installed toolchains\n\
                    --------------------\n\
                    stable-x86_64-pc-windows-msvc (active, default)\n\
                    1.95-x86_64-pc-windows-msvc\n\n\
                    active toolchain\n\
                    ----------------\n\
                    name: stable-x86_64-pc-windows-msvc\n\
                    active because: it's the default toolchain\n\
                    installed targets:\n\
                      aarch64-apple-darwin\n\
                      x86_64-pc-windows-msvc\n";
        let targets = "aarch64-apple-darwin\nx86_64-pc-windows-msvc\n";
        let facts = parse_rustup(show, targets);
        assert_eq!(
            facts.get("host_triple").unwrap(),
            &FactValue::Str("x86_64-pc-windows-msvc".to_string())
        );
        assert_eq!(
            facts.get("active_toolchain").unwrap(),
            &FactValue::Str("stable-x86_64-pc-windows-msvc".to_string())
        );
        assert_eq!(
            facts.get("toolchains").unwrap(),
            &FactValue::Set(vec![
                FactValue::Str("stable-x86_64-pc-windows-msvc".to_string()),
                FactValue::Str("1.95-x86_64-pc-windows-msvc".to_string()),
            ])
        );
        assert_eq!(
            facts.get("targets").unwrap(),
            &FactValue::Set(vec![
                FactValue::Str("aarch64-apple-darwin".to_string()),
                FactValue::Str("x86_64-pc-windows-msvc".to_string()),
            ])
        );
    }

    #[test]
    fn parse_english_rustup() {
        // 英文系统
        let show = "Default host: x86_64-pc-windows-msvc\n\
                    Installed toolchains\n\
                    --------------------\n\
                    stable-x86_64-pc-windows-msvc (active, default)\n\
                    Active toolchain\n\
                    ----------------\n\
                    name: stable-x86_64-pc-windows-msvc\n";
        let facts = parse_rustup(show, "");
        assert_eq!(
            facts.get("active_toolchain").unwrap(),
            &FactValue::Str("stable-x86_64-pc-windows-msvc".to_string())
        );
        assert_eq!(
            facts.get("toolchains").unwrap(),
            &FactValue::Set(vec![FactValue::Str(
                "stable-x86_64-pc-windows-msvc".to_string()
            )])
        );
        assert!(!facts.contains_key("targets"));
    }

    #[test]
    fn parse_garbage_input() {
        // 畸形输入：不 panic
        let facts = parse_rustup("!!! not rustup output !!!\n===garbage===\n", "");
        assert!(!facts.contains_key("host_triple"));
        assert!(!facts.contains_key("toolchains"));
    }
}
