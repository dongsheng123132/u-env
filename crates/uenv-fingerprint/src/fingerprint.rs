// 指纹计算（规格 §5.3）。
// host_input / toolchain_input / project_input / full_input 四个 sha256。
// status == Error 的 detector 不参与指纹，但列出 excluded_detectors。

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use uenv_core::{DetectStatus, Environment, EnvironmentFingerprint, FactValue, Layer};

use crate::normalize::{canonical_json, normalize_fact_value};

/// 计算环境指纹。返回指纹 + 被排除的 detector id 列表。
pub fn compute_fingerprint(
    env: &Environment,
) -> anyhow::Result<(EnvironmentFingerprint, Vec<String>)> {
    let (host_input, host_excluded) = layer_input(env, Layer::Host);
    let (toolchain_input, toolchain_excluded) = layer_input(env, Layer::Toolchain);
    let (project_input, project_excluded) = layer_input(env, Layer::Project);

    let host = hash_input(&host_input);
    let toolchain = hash_input(&toolchain_input);
    let project = if project_input.is_empty() {
        None
    } else {
        Some(hash_input(&project_input))
    };

    // full = sha256(canonical({"host":..., "toolchain":..., "project":...}))
    let mut full_map = BTreeMap::new();
    full_map.insert("host".to_string(), FactValue::Str(host.clone()));
    full_map.insert("toolchain".to_string(), FactValue::Str(toolchain.clone()));
    if let Some(p) = &project {
        full_map.insert("project".to_string(), FactValue::Str(p.clone()));
    }
    let full = hash_input(&full_map);

    let fp = EnvironmentFingerprint {
        host,
        toolchain,
        project,
        full,
    };

    let mut excluded = host_excluded;
    excluded.extend(toolchain_excluded);
    excluded.extend(project_excluded);
    excluded.sort();
    excluded.dedup();

    Ok((fp, excluded))
}

/// 取某层的 detector → facts 规范化输入；返回 (input_map, 被排除的 id)
fn layer_input(env: &Environment, layer: Layer) -> (BTreeMap<String, FactValue>, Vec<String>) {
    let mut input = BTreeMap::new();
    let mut excluded = Vec::new();

    for (id, record) in &env.detectors {
        if record.layer != layer {
            continue;
        }
        // status == Error 的 detector 不参与指纹（网络抖动等不改变指纹）
        if record.status == DetectStatus::Error {
            excluded.push(id.clone());
            continue;
        }
        // 只收 facts，不收 volatile / evidence / elapsed_ms / summary / status
        let mut facts = BTreeMap::new();
        for (k, v) in &record.facts {
            if let Some(n) = normalize_fact_value(v) {
                facts.insert(k.clone(), n);
            }
        }
        input.insert(id.clone(), FactValue::Map(facts));
    }

    (input, excluded)
}

/// sha256(canonical_json(input)) → "origin-env:sha256:<64hex>"
fn hash_input(input: &BTreeMap<String, FactValue>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(canonical_json(&FactValue::Map(input.clone())).as_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in digest {
        hex.push_str(&format!("{b:02x}"));
    }
    format!("origin-env:sha256:{hex}")
}

/// 短显示：取 hex 前 12 位
pub fn short(fp: &EnvironmentFingerprint) -> String {
    let full = &fp.full;
    let hex_start = full.rfind("sha256:").map(|i| i + 7).unwrap_or(0);
    let hex = &full[hex_start..];
    format!("{}…", &hex[..12.min(hex.len())])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use uenv_core::{DetectorRecord, OperatingSystem};

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
                    ubr: Some(6199),
                    edition: Some("Core".to_string()),
                    display_version: Some("23H2".to_string()),
                },
                architecture: uenv_core::Architecture::X64,
                project: None,
            },
            detectors,
            fingerprint: None,
        }
    }

    fn record(
        layer: Layer,
        status: DetectStatus,
        facts: BTreeMap<String, FactValue>,
    ) -> DetectorRecord {
        DetectorRecord {
            id: "x".to_string(),
            layer,
            title: "x".to_string(),
            status,
            summary: "x".to_string(),
            facts,
            volatile: BTreeMap::new(),
            evidence: vec![],
            elapsed_ms: 0,
        }
    }

    #[test]
    fn same_input_same_hash() {
        let facts = BTreeMap::from([("enabled".to_string(), FactValue::Bool(true))]);
        let env = env_with(BTreeMap::from([(
            "windows.long-paths".to_string(),
            record(Layer::Host, DetectStatus::Ok, facts.clone()),
        )]));
        let (fp1, _) = compute_fingerprint(&env).unwrap();
        let (fp2, _) = compute_fingerprint(&env).unwrap();
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn change_fact_changes_hash() {
        let mk = |v: bool| {
            env_with(BTreeMap::from([(
                "windows.long-paths".to_string(),
                record(
                    Layer::Host,
                    DetectStatus::Ok,
                    BTreeMap::from([("enabled".to_string(), FactValue::Bool(v))]),
                ),
            )]))
        };
        let (fp1, _) = compute_fingerprint(&mk(true)).unwrap();
        let (fp2, _) = compute_fingerprint(&mk(false)).unwrap();
        assert_ne!(fp1.full, fp2.full);
    }

    #[test]
    fn volatile_evidence_time_not_in_hash() {
        // 核心不变式：只改 volatile/evidence/generated_at/elapsed_ms → hash 不变
        let mut facts = BTreeMap::from([("enabled".to_string(), FactValue::Bool(true))]);
        let mut r1 = record(Layer::Host, DetectStatus::Ok, facts.clone());
        r1.volatile = BTreeMap::from([("free".to_string(), FactValue::Int(12345))]);
        r1.evidence = vec![uenv_core::Evidence {
            kind: uenv_core::EvidenceKind::Command,
            source: "cmd".to_string(),
            exit_code: Some(0),
            excerpt: "out".to_string(),
        }];
        r1.elapsed_ms = 42;

        let mut r2 = record(Layer::Host, DetectStatus::Ok, facts.clone());
        r2.volatile = BTreeMap::from([("free".to_string(), FactValue::Int(99999))]);
        r2.evidence = vec![uenv_core::Evidence {
            kind: uenv_core::EvidenceKind::Registry,
            source: "reg".to_string(),
            exit_code: None,
            excerpt: "other".to_string(),
        }];
        r2.elapsed_ms = 999;

        let mut env1 = env_with(BTreeMap::from([("windows.long-paths".to_string(), r1)]));
        let mut env2 = env_with(BTreeMap::from([("windows.long-paths".to_string(), r2)]));
        env1.generated_at = "2026-01-01T00:00:00Z".to_string();
        env2.generated_at = "2026-08-06T00:00:00Z".to_string();
        // 只让 id 不同避免 BTreeMap key 冲突——这里 key 相同，r1/r2 不同字段
        let (fp1, _) = compute_fingerprint(&env1).unwrap();
        let (fp2, _) = compute_fingerprint(&env2).unwrap();
        assert_eq!(fp1.full, fp2.full);
        let _ = &mut facts;
    }

    #[test]
    fn error_detector_excluded() {
        let facts = BTreeMap::from([("enabled".to_string(), FactValue::Bool(true))]);
        let env = env_with(BTreeMap::from([
            (
                "windows.long-paths".to_string(),
                record(Layer::Host, DetectStatus::Ok, facts.clone()),
            ),
            (
                "net.proxy".to_string(),
                record(Layer::Host, DetectStatus::Error, facts),
            ),
        ]));
        let (fp, excluded) = compute_fingerprint(&env).unwrap();
        assert!(excluded.contains(&"net.proxy".to_string()));
        // excluded 的 detector 不参与指纹：指纹与只有 long-paths 的环境相同
        let env2 = env_with(BTreeMap::from([(
            "windows.long-paths".to_string(),
            record(
                Layer::Host,
                DetectStatus::Ok,
                BTreeMap::from([("enabled".to_string(), FactValue::Bool(true))]),
            ),
        )]));
        let (fp2, _) = compute_fingerprint(&env2).unwrap();
        assert_eq!(fp.host, fp2.host);
    }

    #[test]
    fn set_order_insensitive_list_order_sensitive() {
        // Set 换顺序 → hash 不变
        let set_a = FactValue::Set(vec![
            FactValue::Str("a".to_string()),
            FactValue::Str("b".to_string()),
        ]);
        let set_b = FactValue::Set(vec![
            FactValue::Str("b".to_string()),
            FactValue::Str("a".to_string()),
        ]);
        assert_eq!(canonical_json(&set_a), canonical_json(&set_b));

        // List 换顺序 → hash 变
        let list_a = FactValue::List(vec![
            FactValue::Str("a".to_string()),
            FactValue::Str("b".to_string()),
        ]);
        let list_b = FactValue::List(vec![
            FactValue::Str("b".to_string()),
            FactValue::Str("a".to_string()),
        ]);
        assert_ne!(canonical_json(&list_a), canonical_json(&list_b));
    }
}
