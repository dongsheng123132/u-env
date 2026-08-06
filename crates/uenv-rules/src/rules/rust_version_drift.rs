// rust.version-drift — rust 声明与 active toolchain 不符。
// Error / manual

#[allow(unused_imports)]
use uenv_core::{Environment, Finding, Severity};

use crate::helpers::{drift_satisfies_false, finding};
use crate::Rule;

pub struct RustVersionDrift;

impl Rule for RustVersionDrift {
    fn id(&self) -> &'static str {
        "rust.version-drift"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["project.drift", "toolchain.rust"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        if !drift_satisfies_false(env, "rust") {
            return vec![];
        }
        vec![finding(
            self.id(),
            Severity::Error,
            "Rust 版本不满足项目声明",
            "项目声明了 rust-version（Cargo.toml）或 toolchain（rust-toolchain.toml），但当前 active toolchain 不满足。rust-version 不满足时 cargo 会在构建期直接报「package requires rustc X, found Y」，而 rust-toolchain.toml 本应自动切换——如果没生效，多半是 rustup 没按文件切（比如 toolchain 名写错）。建议 `rustup show` 确认 active，或 `rustup toolchain install <声明版本>`。",
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use uenv_core::{FactValue, Layer};

    fn env_with_drift(satisfied: &str) -> uenv_core::Environment {
        let mut env = crate::test_utils::empty_env();
        let val = match satisfied {
            "true" => FactValue::Bool(true),
            "false" => FactValue::Bool(false),
            _ => FactValue::Str("unknown".to_string()),
        };
        let drift = FactValue::Map(BTreeMap::from([(
            "rust".to_string(),
            FactValue::Map(BTreeMap::from([
                ("declared".to_string(), crate::test_utils::s("1.88")),
                ("actual".to_string(), crate::test_utils::s("1.77.0")),
                ("satisfied".to_string(), val),
            ])),
        )]));
        crate::test_utils::with_detector(&mut env, "project.drift", Layer::Project,
            BTreeMap::from([("drift".to_string(), drift)]));
        env
    }

    #[test]
    fn triggers_on_false() {
        let out = RustVersionDrift.evaluate(&env_with_drift("false"));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Error);
    }

    #[test]
    fn silent_when_satisfied() {
        assert_eq!(RustVersionDrift.evaluate(&env_with_drift("true")).len(), 0);
        assert_eq!(RustVersionDrift.evaluate(&crate::test_utils::empty_env()).len(), 0);
    }
}
