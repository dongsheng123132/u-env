// fs.project-path-has-space — 规则。
// severity: Severity::Info

#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use uenv_core::{DetectStatus, Environment, FactValue, Finding, Layer, Safety, Severity};

use crate::Rule;
use crate::helpers::{FindingExt, finding};

pub struct FsProjectPathHasSpace;

impl Rule for FsProjectPathHasSpace {
    fn id(&self) -> &'static str {
        "fs.project-path-has-space"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["fs.project-location", "project.kind"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        let desc = "项目路径含空格。大多数现代工具能处理，但部分构建脚本（尤其 Makefile、批处理、老版 MSBuild）不引号拼接路径时会断。Rust/Tauri 项目的 linker 调用偶发踩坑。属低概率问题，先记录，遇到诡异路径错误再迁移。";
        let fix_safety = Safety::Manual;
        let fix_explain = "如遇构建脚本路径报错，将项目移到无空格路径";
        let fix_commands: &[&str] = &["echo \"建议路径不含空格，如 C:\\dev\\my-app\""];
        let fix_rollback: &[&str] = &["（无法自动回滚）"];
        let v = crate::helpers::fact_bool(env, "fs.project-location", "path_has_space");
        let Some(true) = v else {
            return vec![];
        };
        if crate::helpers::kind_is(env, "rust") || crate::helpers::kind_is(env, "tauri") {
            vec![
                finding(self.id(), Severity::Info, "项目路径含空格", desc).with_fix(
                    fix_safety,
                    fix_explain,
                    fix_commands,
                    fix_rollback,
                ),
            ]
        } else {
            vec![]
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
            "fs.project-location",
            Layer::Host,
            BTreeMap::from([("path_has_space".to_string(), crate::test_utils::b(true))]),
        );
        crate::test_utils::with_project_kinds(&mut env, &["rust"]);
        assert_eq!(FsProjectPathHasSpace.evaluate(&env).len(), 1);
    }
}
