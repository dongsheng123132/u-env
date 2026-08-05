// project.drift detector — 声明版本 vs 实际版本的差异（T3 重点）。
// layer=Project
// 声明来源：package.json engines、.nvmrc/.node-version、rust-toolchain.toml、Cargo.toml rust-version。
// 实际版本：跑对应工具版本命令（node/npm/rust/python/pnpm/yarn/bun）。
// 版本区间判断手写（不引 semver crate，白名单外）：
//   24.5.0 / >=22 / ^22.1.0 / ~1.88 / 22.x / >=20 <23；解析不了 → satisfied: unknown。

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, EvidenceKind, FactValue, Layer};

use crate::context::{ScanContext, evidence_from_command};
use crate::detector::{Detector, DetectorMeta, DetectorResult};

/// 工具名 → 版本命令
const TOOL_COMMANDS: &[(&str, &str, &[&str])] = &[
    ("node", "node", &["--version"]),
    ("npm", "npm", &["--version"]),
    ("rust", "rustc", &["--version"]),
    ("python", "python", &["--version"]),
    ("pnpm", "pnpm", &["--version"]),
    ("yarn", "yarn", &["--version"]),
    ("bun", "bun", &["--version"]),
];

pub struct ProjectDrift;

impl Detector for ProjectDrift {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "project.drift",
            layer: Layer::Project,
            title: "声明与实际的漂移",
            cost: Cost::Slow,
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

        // 1. 收集声明（复用 manifests 的解析）
        let mut declared: BTreeMap<String, String> = BTreeMap::new();

        let pkg_path = root.join("package.json");
        if pkg_path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&pkg_path) {
                let (d, _) = crate::detectors::project_manifests::parse_package_json(&content);
                declared.extend(d);
            }
        }
        let cargo_path = root.join("Cargo.toml");
        if cargo_path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&cargo_path) {
                if let Some(rs) = crate::detectors::project_manifests::parse_cargo_toml(&content) {
                    declared.insert("rust".to_string(), rs);
                }
            }
        }
        for f in [".nvmrc", ".node-version"] {
            let p = root.join(f);
            if p.is_file() {
                if let Ok(content) = std::fs::read_to_string(&p) {
                    let v = content.trim();
                    if !v.is_empty() {
                        declared.insert("node".to_string(), v.to_string());
                    }
                }
            }
        }
        for f in ["rust-toolchain.toml", "rust-toolchain"] {
            let p = root.join(f);
            if p.is_file() {
                if let Ok(content) = std::fs::read_to_string(&p) {
                    for line in content.lines() {
                        let t = line.trim();
                        if let Some(rest) = t.strip_prefix("channel") {
                            if let Some(eq) = rest.find('=') {
                                let v = rest[eq + 1..].trim().trim_matches('"');
                                if !v.is_empty() {
                                    declared.insert("rust".to_string(), v.to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        // 无声明 → Skipped（drift 无从谈起）
        if declared.is_empty() {
            return DetectorResult {
                status: DetectStatus::Skipped,
                summary: "项目无工具链声明".to_string(),
                facts: BTreeMap::new(),
                volatile: BTreeMap::new(),
                evidence: vec![],
            };
        }

        // 2. 逐工具探测实际版本
        let mut evidence = Vec::new();
        let mut drift_map: BTreeMap<String, FactValue> = BTreeMap::new();

        for (tool, program, args) in TOOL_COMMANDS {
            let Some(range) = declared.get(*tool) else {
                continue;
            };
            let out = if matches!(*tool, "npm") {
                ctx.run_slow(program, args)
            } else {
                ctx.run(program, args)
            };
            evidence.push(evidence_from_command(
                EvidenceKind::Command,
                &format!("{program} {}", args.join(" ")),
                &out,
            ));

            let actual = parse_actual_version(tool, &out.stdout);
            let satisfied = match &actual {
                Some(a) => version_satisfies(a, range),
                None => Satisfied::Unknown,
            };

            let mut m = BTreeMap::new();
            m.insert("declared".to_string(), FactValue::Str(range.clone()));
            if let Some(a) = &actual {
                m.insert("actual".to_string(), FactValue::Version(a.clone()));
            }
            m.insert(
                "satisfied".to_string(),
                match satisfied {
                    Satisfied::Yes => FactValue::Bool(true),
                    Satisfied::No => FactValue::Bool(false),
                    Satisfied::Unknown => FactValue::Str("unknown".to_string()),
                },
            );
            drift_map.insert(tool.to_string(), FactValue::Map(m));
        }

        let drift_count = drift_map.len();
        let mut facts = BTreeMap::new();
        facts.insert("drift".to_string(), FactValue::Map(drift_map));

        DetectorResult {
            status: DetectStatus::Ok,
            summary: format!("{drift_count} 个工具声明比对"),
            facts,
            volatile: BTreeMap::new(),
            evidence,
        }
    }
}

#[derive(Debug, PartialEq)]
enum Satisfied {
    Yes,
    No,
    Unknown,
}

/// 从版本命令输出提取实际版本号
fn parse_actual_version(tool: &str, stdout: &str) -> Option<String> {
    let t = stdout.trim();
    if t.is_empty() {
        return None;
    }
    let v = match tool {
        "node" | "npm" | "pnpm" | "yarn" | "bun" => t
            .trim_start_matches('v')
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string(),
        // rustc --version: "rustc 1.88.0 (a1b2c3 2025-01-01)"
        "rust" => {
            let mut parts = t.split_whitespace();
            let _ = parts.next(); // rustc
            parts.next().unwrap_or("").to_string()
        }
        // python --version: "Python 3.12.7"
        "python" => t
            .strip_prefix("Python ")
            .map(|s| s.split_whitespace().next().unwrap_or("").to_string())
            .unwrap_or_default(),
        _ => t.to_string(),
    };
    if v.is_empty() { None } else { Some(v) }
}

/// 实际版本是否满足声明区间
fn version_satisfies(actual: &str, range: &str) -> Satisfied {
    let Some(act) = parse_version(actual) else {
        return Satisfied::Unknown;
    };
    let range = range.trim();

    // 单边界：>=x / <=x
    if let Some(lo) = range.strip_prefix(">=") {
        let lo_v = match parse_version(lo) {
            Some(v) => v,
            None => return Satisfied::Unknown,
        };
        if range.contains('<') {
            // ">=20 <23" 双边界（也可能是 ">=20 <23.0.0"）
            if let Some(hi_part) = range.split('<').nth(1) {
                if let Some(hi_v) = parse_version(hi_part) {
                    return if ge(act, lo_v) && lt(act, hi_v) {
                        Satisfied::Yes
                    } else {
                        Satisfied::No
                    };
                }
            }
        }
        return if ge(act, lo_v) {
            Satisfied::Yes
        } else {
            Satisfied::No
        };
    }
    if let Some(hi) = range.strip_prefix("<=") {
        let hi_v = match parse_version(hi) {
            Some(v) => v,
            None => return Satisfied::Unknown,
        };
        return if le(act, hi_v) {
            Satisfied::Yes
        } else {
            Satisfied::No
        };
    }

    // 22.x / 22.* → >=22.0.0 <23.0.0（必须在精确匹配前判断，否则 "22.x" 会被 parse_version 吃掉）
    if let Some(major_str) = range
        .strip_suffix(".x")
        .or_else(|| range.strip_suffix(".*"))
    {
        if let Ok(major) = major_str.trim().parse::<i64>() {
            let lo = Version {
                major,
                minor: 0,
                patch: 0,
            };
            let hi = Version {
                major: major + 1,
                minor: 0,
                patch: 0,
            };
            return if ge(act, lo) && lt(act, hi) {
                Satisfied::Yes
            } else {
                Satisfied::No
            };
        }
        return Satisfied::Unknown;
    }

    // 精确：24.5.0
    if let Some(exact) = parse_version(range) {
        return if eq(act, exact) {
            Satisfied::Yes
        } else {
            Satisfied::No
        };
    }

    // ^22.1.0 → >=22.1.0 <23.0.0
    if let Some(rest) = range.strip_prefix('^') {
        if let Some(base) = parse_version(rest) {
            let lo = base;
            let hi = Version {
                major: base.major + 1,
                minor: 0,
                patch: 0,
            };
            return if ge(act, lo) && lt(act, hi) {
                Satisfied::Yes
            } else {
                Satisfied::No
            };
        }
        return Satisfied::Unknown;
    }

    // ~1.88 → >=1.88.0 <1.89.0；~1.88.5 → >=1.88.5 <1.89.0
    if let Some(rest) = range.strip_prefix('~') {
        if let Some(base) = parse_version(rest) {
            let lo = base;
            let hi = Version {
                major: base.major,
                minor: base.minor + 1,
                patch: 0,
            };
            return if ge(act, lo) && lt(act, hi) {
                Satisfied::Yes
            } else {
                Satisfied::No
            };
        }
        return Satisfied::Unknown;
    }

    // 解析不了（含 "stable" 这类非版本）→ unknown
    Satisfied::Unknown
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Version {
    major: i64,
    minor: i64,
    patch: i64,
}

/// 解析版本号：取前三个数字段；非数字尾（如 -beta）忽略
fn parse_version(s: &str) -> Option<Version> {
    let s = s.trim();
    let num_part = s
        .split(|c: char| !c.is_ascii_digit() && c != '.')
        .next()
        .unwrap_or("");
    let parts: Vec<&str> = num_part.split('.').collect();
    if parts.is_empty() || parts[0].is_empty() {
        return None;
    }
    let major = parts[0].parse::<i64>().ok()?;
    let minor = parts
        .get(1)
        .and_then(|p| p.parse::<i64>().ok())
        .unwrap_or(0);
    let patch = parts
        .get(2)
        .and_then(|p| p.parse::<i64>().ok())
        .unwrap_or(0);
    Some(Version {
        major,
        minor,
        patch,
    })
}

/// 版本比较
fn cmp(a: Version, b: Version) -> std::cmp::Ordering {
    a.major
        .cmp(&b.major)
        .then(a.minor.cmp(&b.minor))
        .then(a.patch.cmp(&b.patch))
}

fn ge(act: Version, bound: Version) -> bool {
    !matches!(cmp(act, bound), std::cmp::Ordering::Less)
}
fn lt(act: Version, bound: Version) -> bool {
    matches!(cmp(act, bound), std::cmp::Ordering::Less)
}
fn le(act: Version, bound: Version) -> bool {
    !matches!(cmp(act, bound), std::cmp::Ordering::Greater)
}
fn eq(act: Version, bound: Version) -> bool {
    matches!(cmp(act, bound), std::cmp::Ordering::Equal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match() {
        assert_eq!(version_satisfies("24.5.0", "24.5.0"), Satisfied::Yes);
        assert_eq!(version_satisfies("24.5.1", "24.5.0"), Satisfied::No);
    }

    #[test]
    fn greater_than_equal() {
        assert_eq!(version_satisfies("22.1.0", ">=22"), Satisfied::Yes);
        assert_eq!(version_satisfies("22.0.0", ">=22"), Satisfied::Yes);
        assert_eq!(version_satisfies("21.9.0", ">=22"), Satisfied::No);
    }

    #[test]
    fn caret_range() {
        // ^22.1.0 → >=22.1.0 <23.0.0
        assert_eq!(version_satisfies("22.5.0", "^22.1.0"), Satisfied::Yes);
        assert_eq!(version_satisfies("22.0.9", "^22.1.0"), Satisfied::No);
        assert_eq!(version_satisfies("23.0.0", "^22.1.0"), Satisfied::No);
    }

    #[test]
    fn tilde_range() {
        // ~1.88 → >=1.88.0 <1.89.0
        assert_eq!(version_satisfies("1.88.5", "~1.88"), Satisfied::Yes);
        assert_eq!(version_satisfies("1.89.0", "~1.88"), Satisfied::No);
    }

    #[test]
    fn x_range() {
        assert_eq!(version_satisfies("22.9.9", "22.x"), Satisfied::Yes);
        assert_eq!(version_satisfies("23.0.0", "22.x"), Satisfied::No);
    }

    #[test]
    fn double_boundary() {
        assert_eq!(version_satisfies("22.5.0", ">=20 <23"), Satisfied::Yes);
        assert_eq!(version_satisfies("23.5.0", ">=20 <23"), Satisfied::No);
        assert_eq!(version_satisfies("19.0.0", ">=20 <23"), Satisfied::No);
    }

    #[test]
    fn unknown_range() {
        assert_eq!(version_satisfies("1.88.0", "stable"), Satisfied::Unknown);
        assert_eq!(version_satisfies("1.88.0", "lts/*"), Satisfied::Unknown);
    }

    #[test]
    fn actual_version_parsing() {
        assert_eq!(
            parse_actual_version("node", "v22.14.0\r\n").as_deref(),
            Some("22.14.0")
        );
        assert_eq!(
            parse_actual_version("rust", "rustc 1.88.0 (a1b2c3 2025-01-01)\r\n").as_deref(),
            Some("1.88.0")
        );
        assert_eq!(
            parse_actual_version("python", "Python 3.12.7\r\n").as_deref(),
            Some("3.12.7")
        );
        assert_eq!(
            parse_actual_version("npm", "10.9.2\r\n").as_deref(),
            Some("10.9.2")
        );
        assert_eq!(parse_actual_version("node", ""), None);
    }
}
