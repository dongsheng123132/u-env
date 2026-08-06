// uenv-adapters — 框架 adapter 元数据层。
// Adapter 的职责：声明一个框架需要哪些 capability、依赖哪些 detector 的哪些 fact。
// T3 阶段不做诊断（诊断在 T5 规则引擎），只提供元数据 + 供 T5 消费的结构。
//
// 依赖方向：uenv-adapters → uenv-core（根）。不依赖 scanner，无环。

pub mod electron;
pub mod node;
pub mod rust;
pub mod tauri;

use uenv_core::{Environment, FactValue};

/// Adapter 元数据
pub struct AdapterMeta {
    pub id: &'static str,
    pub required_capabilities: &'static [&'static str],
    pub relevant_detectors: &'static [&'static str],
}

/// Adapter trait —— 外部贡献者可实现新框架
pub trait Adapter: Send + Sync {
    fn meta(&self) -> AdapterMeta;
    /// 是否匹配当前环境：看 project.kind 的 facts
    fn matches(&self, env: &Environment) -> bool;
}

/// 全部内置 adapter（显式列表，同 registry 风格）
pub fn all_adapters() -> Vec<Box<dyn Adapter>> {
    vec![
        Box::new(tauri::TauriAdapter),
        Box::new(electron::ElectronAdapter),
        Box::new(node::NodeAdapter),
        Box::new(rust::RustAdapter),
    ]
}

/// 从 Environment 的 project.kind facts 里取 kinds 集合
fn project_kinds(env: &Environment) -> Vec<String> {
    let Some(record) = env.detectors.get("project.kind") else {
        return vec![];
    };
    let Some(FactValue::Set(kinds)) = record.facts.get("kinds") else {
        return vec![];
    };
    kinds
        .iter()
        .filter_map(|k| match k {
            uenv_core::FactValue::Str(s) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

/// 环境是否属于指定项目类型
pub fn env_has_kind(env: &Environment, kind: &str) -> bool {
    project_kinds(env).iter().any(|k| k == kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use uenv_core::{DetectStatus, DetectorRecord, FactValue, Layer};

    fn fake_env(kinds: &[&str]) -> Environment {
        let facts = BTreeMap::from([(
            "kinds".to_string(),
            FactValue::Set(
                kinds
                    .iter()
                    .map(|k| FactValue::Str(k.to_string()))
                    .collect(),
            ),
        )]);
        let record = DetectorRecord {
            id: "project.kind".to_string(),
            layer: Layer::Project,
            title: "项目类型".to_string(),
            status: DetectStatus::Ok,
            summary: String::new(),
            facts,
            volatile: BTreeMap::new(),
            evidence: vec![],
            elapsed_ms: 0,
        };
        Environment {
            spec: String::new(),
            generated_at: String::new(),
            uenv_version: String::new(),
            identity: uenv_core::EnvironmentIdentity {
                host_alias: String::new(),
                os: uenv_core::OperatingSystem {
                    family: String::new(),
                    product_name: String::new(),
                    product_name_raw: String::new(),
                    version: String::new(),
                    build: 0,
                    ubr: None,
                    edition: None,
                    display_version: None,
                },
                architecture: uenv_core::Architecture::X64,
                project: None,
            },
            detectors: BTreeMap::from([("project.kind".to_string(), record)]),
            fingerprint: None,
        }
    }

    #[test]
    fn tauri_matches_tauri_project() {
        let env = fake_env(&["tauri", "rust", "node"]);
        assert!(tauri::TauriAdapter.matches(&env));
        assert!(!electron::ElectronAdapter.matches(&env));
        assert!(node::NodeAdapter.matches(&env));
        assert!(rust::RustAdapter.matches(&env));
    }

    #[test]
    fn electron_matches_electron_project() {
        let env = fake_env(&["electron", "node"]);
        assert!(electron::ElectronAdapter.matches(&env));
        assert!(!tauri::TauriAdapter.matches(&env));
        assert!(!rust::RustAdapter.matches(&env));
    }

    #[test]
    fn rust_adapter_matches_plain_rust() {
        let env = fake_env(&["rust"]);
        assert!(!tauri::TauriAdapter.matches(&env));
        assert!(!electron::ElectronAdapter.matches(&env));
        assert!(!node::NodeAdapter.matches(&env));
        assert!(rust::RustAdapter.matches(&env));
    }

    #[test]
    fn all_adapters_meta_valid() {
        for a in all_adapters() {
            let m = a.meta();
            assert!(!m.id.is_empty());
            assert!(!m.required_capabilities.is_empty());
            assert!(!m.relevant_detectors.is_empty());
        }
    }
}
