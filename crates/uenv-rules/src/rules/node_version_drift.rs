// node.version-drift — project.drift 里 node 的 satisfied == false。
// Error / manual

#[allow(unused_imports)]
use uenv_core::{Environment, Finding, Severity};

use crate::helpers::{drift_satisfies_false, finding};
use crate::Rule;

pub struct NodeVersionDrift;

impl Rule for NodeVersionDrift {
    fn id(&self) -> &'static str {
        "node.version-drift"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["project.drift", "toolchain.node"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        if !drift_satisfies_false(env, "node") {
            return vec![];
        }
        vec![finding(
            self.id(),
            Severity::Error,
            "Node 版本不满足项目声明",
            "项目声明了 Node 版本范围（package.json engines / .nvmrc），但当前 PATH 里生效的 node 不在这个范围内。npx / npm scripts 里若依赖 Node 特性（如 fetch、ESM、特定 API），版本不对会直接报错或者更糟——行为诡异但能跑。先看 project.drift 里 declared 和 actual 的差值，再决定升级还是用 nvm-windows/fnm 切版本。",
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
            "node".to_string(),
            FactValue::Map(BTreeMap::from([
                ("declared".to_string(), crate::test_utils::s(">=22")),
                ("actual".to_string(), crate::test_utils::s("20.0.0")),
                ("satisfied".to_string(), val),
            ])),
        )]));
        crate::test_utils::with_detector(&mut env, "project.drift", Layer::Project,
            BTreeMap::from([("drift".to_string(), drift)]));
        env
    }

    #[test]
    fn triggers_on_false() {
        let out = NodeVersionDrift.evaluate(&env_with_drift("false"));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Error);
    }

    #[test]
    fn silent_when_satisfied_or_unknown() {
        assert_eq!(NodeVersionDrift.evaluate(&env_with_drift("true")).len(), 0);
        assert_eq!(NodeVersionDrift.evaluate(&env_with_drift("unknown")).len(), 0);
    }
}
