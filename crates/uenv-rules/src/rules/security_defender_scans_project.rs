// security.defender-scans-project — 规则。
// severity: Severity::Info

#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use uenv_core::{DetectStatus, Environment, FactValue, Finding, Layer, Safety, Severity};

use crate::Rule;
use crate::helpers::{FindingExt, finding};

pub struct SecurityDefenderScansProject;

impl Rule for SecurityDefenderScansProject {
    fn id(&self) -> &'static str {
        "security.defender-scans-project"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["security.defender", "fs.project-location"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        let desc = "Defender 实时保护开着，但排除项没覆盖项目目录。node_modules/cargo target 这类海量小文件每次构建都被实时扫描，编译时间可能慢 30-50%（尤其 Rust 增量编译）。把项目目录和工具链目录加进 Defender 排除项能显著提速——注意只排除你信任的开发目录。";
        let fix_safety = Safety::Manual;
        let fix_explain = "把项目目录加入 Defender 排除项（需管理员；Add-MpPreference -ExclusionPath '<项目路径>' 加排除，Remove-MpPreference -ExclusionPath '<项目路径>' 即回滚）。涉及安全软件配置，本工具不代执行";
        let rt = crate::helpers::fact_bool(env, "security.defender", "realtime_enabled");
        let Some(true) = rt else {
            return vec![];
        };
        let covers =
            crate::helpers::fact_bool(env, "security.defender", "exclusion_covers_project");
        match covers {
            Some(true) => vec![],
            _ => vec![
                finding(
                    self.id(),
                    Severity::Info,
                    "Defender 实时保护未排除项目目录",
                    desc,
                )
                .with_fix(fix_safety, fix_explain),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_triggers_and_silent() {
        let mut env = crate::test_utils::empty_env();
        crate::test_utils::with_detector(
            &mut env,
            "security.defender",
            Layer::Host,
            BTreeMap::from([
                ("realtime_enabled".to_string(), crate::test_utils::b(true)),
                (
                    "exclusion_covers_project".to_string(),
                    crate::test_utils::b(false),
                ),
            ]),
        );
        assert_eq!(SecurityDefenderScansProject.evaluate(&env).len(), 1);

        let mut env2 = crate::test_utils::empty_env();
        crate::test_utils::with_detector(
            &mut env2,
            "security.defender",
            Layer::Host,
            BTreeMap::from([
                ("realtime_enabled".to_string(), crate::test_utils::b(true)),
                (
                    "exclusion_covers_project".to_string(),
                    crate::test_utils::b(true),
                ),
            ]),
        );
        assert_eq!(SecurityDefenderScansProject.evaluate(&env2).len(), 0);
    }
}
