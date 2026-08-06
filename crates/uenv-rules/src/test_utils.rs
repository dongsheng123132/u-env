// 测试工具：构造 Environment fixture。仅在测试中使用。

use std::collections::BTreeMap;

use uenv_core::{
    DetectStatus, DetectorRecord, Environment, EnvironmentIdentity, FactValue, Layer,
    OperatingSystem,
};

/// 空环境
pub fn empty_env() -> Environment {
    Environment {
        spec: "origin-environment/v0.1".to_string(),
        generated_at: "2026-08-06T00:00:00Z".to_string(),
        uenv_version: "0.0.1".to_string(),
        identity: EnvironmentIdentity {
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
        detectors: BTreeMap::new(),
        fingerprint: None,
    }
}

/// 加一个 detector record（ok 状态，指定 facts）
pub fn with_detector(
    env: &mut Environment,
    id: &str,
    layer: Layer,
    facts: BTreeMap<String, FactValue>,
) {
    env.detectors.insert(
        id.to_string(),
        DetectorRecord {
            id: id.to_string(),
            layer,
            title: id.to_string(),
            status: DetectStatus::Ok,
            summary: String::new(),
            facts,
            volatile: BTreeMap::new(),
            evidence: vec![],
            elapsed_ms: 0,
        },
    );
}

/// 加一个 Error 状态 detector
pub fn with_error_detector(env: &mut Environment, id: &str) {
    env.detectors.insert(
        id.to_string(),
        DetectorRecord {
            id: id.to_string(),
            layer: Layer::Host,
            title: id.to_string(),
            status: DetectStatus::Error,
            summary: "failed".to_string(),
            facts: BTreeMap::new(),
            volatile: BTreeMap::new(),
            evidence: vec![],
            elapsed_ms: 0,
        },
    );
}

/// 便捷：项目 kinds
pub fn with_project_kinds(env: &mut Environment, kinds: &[&str]) {
    with_detector(
        env,
        "project.kind",
        Layer::Project,
        BTreeMap::from([(
            "kinds".to_string(),
            FactValue::Set(
                kinds
                    .iter()
                    .map(|k| FactValue::Str(k.to_string()))
                    .collect(),
            ),
        )]),
    );
}

pub fn s(v: &str) -> FactValue {
    FactValue::Str(v.to_string())
}
pub fn b(v: bool) -> FactValue {
    FactValue::Bool(v)
}
pub fn i(v: i64) -> FactValue {
    FactValue::Int(v)
}
pub fn set_str(items: &[&str]) -> FactValue {
    FactValue::Set(
        items
            .iter()
            .map(|s| FactValue::Str(s.to_string()))
            .collect(),
    )
}
