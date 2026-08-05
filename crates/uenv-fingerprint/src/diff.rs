// 环境差异（规格 §5.5）：按 detector → fact 键逐层比较，三段输出。
// 风险分级来自 uenv-rules 的 critical_fact_keys()，diff 不自己猜。

use std::collections::BTreeMap;

use uenv_core::{DetectStatus, Environment, FactValue};
use uenv_rules::is_critical_fact;

use crate::normalize::{canonical_json, normalize_fact_value};

/// 风险等级
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Risk {
    High,
    Low,
}

/// 单条差异
#[derive(Debug, Clone, PartialEq)]
pub struct FactDiff {
    pub detector: String,
    pub key: String,
    pub a: String,
    pub b: String,
    pub risk: Risk,
}

/// 计算两份环境的语义差异（逐 detector → fact 键，规范化后比较）。
/// 返回 (高风险的差异, 低风险的差异, 仅一侧存在的 detector id)。
pub fn diff_environments(
    a: &Environment,
    b: &Environment,
) -> (Vec<FactDiff>, Vec<FactDiff>, Vec<String>) {
    let mut high: Vec<FactDiff> = Vec::new();
    let mut low: Vec<FactDiff> = Vec::new();

    let mut all_ids: Vec<&String> = Vec::new();
    for id in a.detectors.keys() {
        all_ids.push(id);
    }
    for id in b.detectors.keys() {
        if !all_ids.contains(&id) {
            all_ids.push(id);
        }
    }
    all_ids.sort();

    let mut only_one_side: Vec<String> = Vec::new();

    for id in &all_ids {
        let ra = a.detectors.get(*id);
        let rb = b.detectors.get(*id);

        // 仅一侧存在：两侧 status 都非 Error 才报（Error 不参与指纹，也不该制造噪音）
        let a_active = ra.is_some_and(|r| r.status != DetectStatus::Error);
        let b_active = rb.is_some_and(|r| r.status != DetectStatus::Error);
        if ra.is_none() && b_active {
            only_one_side.push((*id).clone());
            continue;
        }
        if rb.is_none() && a_active {
            only_one_side.push((*id).clone());
            continue;
        }

        let (Some(ra), Some(rb)) = (ra, rb) else {
            continue;
        };

        // 规范化两侧 facts（volatile/evidence/elapsed_ms 永不参与比较）
        let facts_a = normalized_facts(&ra.facts);
        let facts_b = normalized_facts(&rb.facts);

        let mut keys: Vec<&String> = Vec::new();
        for k in facts_a.keys() {
            keys.push(k);
        }
        for k in facts_b.keys() {
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
        keys.sort();

        for key in keys {
            let va = facts_a.get(key);
            let vb = facts_b.get(key);
            match (va, vb) {
                (Some(x), Some(y)) => {
                    if canonical_json(x) != canonical_json(y) {
                        let d = FactDiff {
                            detector: (*id).clone(),
                            key: key.clone(),
                            a: canonical_json(x),
                            b: canonical_json(y),
                            risk: if is_critical_fact(id, key) {
                                Risk::High
                            } else {
                                Risk::Low
                            },
                        };
                        if d.risk == Risk::High {
                            high.push(d);
                        } else {
                            low.push(d);
                        }
                    }
                }
                (Some(x), None) | (None, Some(x)) => {
                    // 一侧有键另一侧没有 → 差异（用空串表示缺失）
                    let (a_val, b_val) = if va.is_some() {
                        (canonical_json(x), String::new())
                    } else {
                        (String::new(), canonical_json(x))
                    };
                    let d = FactDiff {
                        detector: (*id).clone(),
                        key: key.clone(),
                        a: a_val,
                        b: b_val,
                        risk: if is_critical_fact(id, key) {
                            Risk::High
                        } else {
                            Risk::Low
                        },
                    };
                    if d.risk == Risk::High {
                        high.push(d);
                    } else {
                        low.push(d);
                    }
                }
                _ => {}
            }
        }
    }

    (high, low, only_one_side)
}

/// facts 规范化（删空值、Set 排序去重等）
fn normalized_facts(facts: &BTreeMap<String, FactValue>) -> BTreeMap<String, FactValue> {
    let mut out = BTreeMap::new();
    for (k, v) in facts {
        if let Some(n) = normalize_fact_value(v) {
            out.insert(k.clone(), n);
        }
    }
    out
}

/// 人读文本输出（§5.5 格式）
pub fn render_text(high: &[FactDiff], low: &[FactDiff], only_one_side: &[String]) -> String {
    let mut out = String::new();
    out.push_str("高风险差异（影响构建/运行）：\n");
    if high.is_empty() {
        out.push_str("  （无）\n");
    }
    for d in high {
        out.push_str(&format!("- {}: {}  {} → {}\n", d.detector, d.key, d.a, d.b));
    }
    out.push_str("\n低风险差异：\n");
    if low.is_empty() {
        out.push_str("  （无）\n");
    }
    for d in low {
        out.push_str(&format!("- {}: {}  {} → {}\n", d.detector, d.key, d.a, d.b));
    }
    out.push_str("\n仅一侧存在：\n");
    if only_one_side.is_empty() {
        out.push_str("  （无）\n");
    }
    for id in only_one_side {
        out.push_str(&format!("- {id}\n"));
    }
    out
}

/// JSON 结构化输出（数组，每项含 detector/key/a/b/risk）
pub fn render_json(high: &[FactDiff], low: &[FactDiff], only_one_side: &[String]) -> String {
    let mut items: Vec<serde_json::Value> = Vec::new();
    for d in high {
        items.push(diff_to_json(d, "high"));
    }
    for d in low {
        items.push(diff_to_json(d, "low"));
    }
    for id in only_one_side {
        items.push(serde_json::json!({
            "detector": id,
            "key": "*",
            "a": "",
            "b": "",
            "risk": "one_side",
        }));
    }
    serde_json::to_string_pretty(&items).unwrap_or_default()
}

fn diff_to_json(d: &FactDiff, risk: &str) -> serde_json::Value {
    serde_json::json!({
        "detector": d.detector,
        "key": d.key,
        "a": d.a,
        "b": d.b,
        "risk": risk,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use uenv_core::{DetectorRecord, Evidence, EvidenceKind, Layer, OperatingSystem};

    fn record(facts: BTreeMap<String, FactValue>) -> DetectorRecord {
        DetectorRecord {
            id: "x".to_string(),
            layer: Layer::Toolchain,
            title: "x".to_string(),
            status: DetectStatus::Ok,
            summary: "x".to_string(),
            facts,
            volatile: BTreeMap::new(),
            evidence: vec![],
            elapsed_ms: 0,
        }
    }

    fn env_with(detectors: BTreeMap<String, DetectorRecord>) -> Environment {
        Environment {
            spec: "origin-environment/v0.1".to_string(),
            generated_at: "2026-08-06T00:00:00Z".to_string(),
            uenv_version: "0.0.1".to_string(),
            identity: uenv_core::EnvironmentIdentity {
                host_alias: "<host>".to_string(),
                os: OperatingSystem {
                    family: "windows".to_string(),
                    product_name: "Windows 11 Home".to_string(),
                    product_name_raw: "Windows 10 Home".to_string(),
                    version: "10.0.22631".to_string(),
                    build: 22631,
                    ubr: None,
                    edition: None,
                    display_version: None,
                },
                architecture: uenv_core::Architecture::X64,
                project: None,
            },
            detectors,
            fingerprint: None,
        }
    }

    #[test]
    fn diff_identical_envs() {
        let facts = BTreeMap::from([("enabled".to_string(), FactValue::Bool(true))]);
        let env = env_with(BTreeMap::from([(
            "windows.long-paths".to_string(),
            record(facts),
        )]));
        let (high, low, only) = diff_environments(&env, &env);
        assert!(high.is_empty());
        assert!(low.is_empty());
        assert!(only.is_empty());
    }

    #[test]
    fn diff_high_risk_node_version() {
        // toolchain.node:versions 是关键键 → 高风险
        let mk = |v: &str| {
            env_with(BTreeMap::from([(
                "toolchain.node".to_string(),
                record(BTreeMap::from([(
                    "versions".to_string(),
                    FactValue::Map(BTreeMap::from([(
                        "primary".to_string(),
                        FactValue::Version(v.to_string()),
                    )])),
                )])),
            )]))
        };
        let (high, low, _) = diff_environments(&mk("22.14.0"), &mk("24.5.0"));
        assert_eq!(high.len(), 1);
        assert_eq!(high[0].detector, "toolchain.node");
        assert_eq!(high[0].key, "versions");
        assert_eq!(high[0].risk, Risk::High);
        assert!(low.is_empty());
    }

    #[test]
    fn diff_low_risk_ubr() {
        // windows.version:ubr 不在关键清单 → 低风险
        let mk = |u: i64| {
            env_with(BTreeMap::from([(
                "windows.version".to_string(),
                record(BTreeMap::from([("ubr".to_string(), FactValue::Int(u))])),
            )]))
        };
        let (high, low, _) = diff_environments(&mk(2454), &mk(2506));
        assert!(high.is_empty());
        assert_eq!(low.len(), 1);
        assert_eq!(low[0].risk, Risk::Low);
    }

    #[test]
    fn diff_volatile_not_compared() {
        // volatile 差异不产生 diff
        let mut r1 = record(BTreeMap::from([(
            "enabled".to_string(),
            FactValue::Bool(true),
        )]));
        r1.volatile = BTreeMap::from([("free".to_string(), FactValue::Int(1))]);
        let mut r2 = record(BTreeMap::from([(
            "enabled".to_string(),
            FactValue::Bool(true),
        )]));
        r2.volatile = BTreeMap::from([("free".to_string(), FactValue::Int(999))]);
        let env1 = env_with(BTreeMap::from([("host.disk".to_string(), r1)]));
        let env2 = env_with(BTreeMap::from([("host.disk".to_string(), r2)]));
        let (high, low, _) = diff_environments(&env1, &env2);
        assert!(high.is_empty());
        assert!(low.is_empty());
    }

    #[test]
    fn diff_missing_detector_one_side() {
        let env1 = env_with(BTreeMap::from([(
            "toolchain.node".to_string(),
            record(BTreeMap::new()),
        )]));
        let env2 = env_with(BTreeMap::new());
        let (_, _, only) = diff_environments(&env1, &env2);
        assert!(only.contains(&"toolchain.node".to_string()));
    }

    #[test]
    fn diff_key_only_one_side() {
        // 一侧有 fact 键，另一侧没有
        let env1 = env_with(BTreeMap::from([(
            "toolchain.rust".to_string(),
            record(BTreeMap::from([(
                "active_toolchain".to_string(),
                FactValue::Str("stable".to_string()),
            )])),
        )]));
        let env2 = env_with(BTreeMap::from([(
            "toolchain.rust".to_string(),
            record(BTreeMap::new()),
        )]));
        let (high, low, _) = diff_environments(&env1, &env2);
        // active_toolchain 是关键键 → 高风险
        assert_eq!(high.len(), 1);
        assert_eq!(high[0].key, "active_toolchain");
        assert!(low.is_empty());
    }

    #[test]
    fn evidence_never_diff() {
        // 只改 evidence → 无 diff
        let mut r1 = record(BTreeMap::from([(
            "enabled".to_string(),
            FactValue::Bool(true),
        )]));
        r1.evidence = vec![Evidence {
            kind: EvidenceKind::Command,
            source: "a".to_string(),
            exit_code: Some(0),
            excerpt: "x".to_string(),
        }];
        let mut r2 = record(BTreeMap::from([(
            "enabled".to_string(),
            FactValue::Bool(true),
        )]));
        r2.evidence = vec![Evidence {
            kind: EvidenceKind::Registry,
            source: "b".to_string(),
            exit_code: None,
            excerpt: "y".to_string(),
        }];
        let env1 = env_with(BTreeMap::from([("windows.long-paths".to_string(), r1)]));
        let env2 = env_with(BTreeMap::from([("windows.long-paths".to_string(), r2)]));
        let (high, low, _) = diff_environments(&env1, &env2);
        assert!(high.is_empty());
        assert!(low.is_empty());
    }
}
