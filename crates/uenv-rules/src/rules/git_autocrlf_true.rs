// git.autocrlf-true — 规则。
// severity: Severity::Warning

#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use uenv_core::{DetectStatus, Environment, FactValue, Finding, Layer, Safety, Severity};

use crate::Rule;
use crate::helpers::{FindingExt, finding};

pub struct GitAutocrlfTrue;

impl Rule for GitAutocrlfTrue {
    fn id(&self) -> &'static str {
        "git.autocrlf-true"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["toolchain.git", "project.kind"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        let desc = "core.autocrlf=true 会在 checkout 时把 LF 转 CRLF。Rust 项目（尤其带 .sh 脚本、Makefile、或者被 CI 拉取在 Linux 上构建的仓库）会因为行尾转换产生 diff 噪音甚至脚本执行错误。建议对仓库用 .gitattributes 显式声明，或对代码仓库设 autocrlf=input/false。";
        let fix_safety = Safety::Confirm;
        let fix_explain = "对本仓库关闭 autocrlf 或改用 input。触发条件钉住了旧值——本条只在当前 core.autocrlf=true 时触发，rollback 设回 true 即还原";
        if !crate::helpers::kind_is(env, "rust") {
            return vec![];
        }
        let v = crate::helpers::fact_str(env, "toolchain.git", "autocrlf");
        let Some(v) = v else {
            return vec![];
        };
        if v.eq_ignore_ascii_case("true") {
            vec![
                finding(
                    self.id(),
                    Severity::Warning,
                    "git core.autocrlf=true 对 Rust 项目有风险",
                    desc,
                )
                .with_fix(fix_safety, fix_explain),
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
        crate::test_utils::with_project_kinds(&mut env, &["rust"]);
        crate::test_utils::with_detector(
            &mut env,
            "toolchain.git",
            Layer::Toolchain,
            BTreeMap::from([("autocrlf".to_string(), crate::test_utils::s("true"))]),
        );
        assert_eq!(GitAutocrlfTrue.evaluate(&env).len(), 1);

        let mut env2 = crate::test_utils::empty_env();
        crate::test_utils::with_project_kinds(&mut env2, &["rust"]);
        crate::test_utils::with_detector(
            &mut env2,
            "toolchain.git",
            Layer::Toolchain,
            BTreeMap::from([("autocrlf".to_string(), crate::test_utils::s("input"))]),
        );
        assert_eq!(GitAutocrlfTrue.evaluate(&env2).len(), 0);
    }
}
