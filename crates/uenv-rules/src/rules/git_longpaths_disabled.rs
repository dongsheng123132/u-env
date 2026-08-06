// git.longpaths-disabled — 规则。
// severity: Severity::Warning

#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use uenv_core::{DetectStatus, Environment, FactValue, Finding, Layer, Safety, Severity};

use crate::helpers::{finding, FindingExt};
use crate::Rule;

pub struct GitLongpathsDisabled;

impl Rule for GitLongpathsDisabled {
    fn id(&self) -> &'static str {
        "git.longpaths-disabled"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["toolchain.git"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        let desc = "git 默认也受 260 字符路径限制。Node 项目 node_modules 里的深层路径在 git add/checkout 时报「Filename too long」。开启后对已有仓库需重新 checkout 才生效。";
        let fix_safety = Safety::Confirm;
        let fix_explain = "开启 git 长路径支持";
        let fix_commands: &[&str] = &["git config --global core.longpaths true"];
        let fix_rollback: &[&str] = &["git config --global --unset core.longpaths"];
        let v = crate::helpers::fact_str(env, "toolchain.git", "longpaths");
        let Some(v) = v else {
            // 未配置默认关 → 触发（保守：宁报勿漏）
            return vec![finding(self.id(), Severity::Warning, "git core.longpaths 未开启", desc).with_fix(fix_safety, fix_explain, fix_commands, fix_rollback)];
        };
        if !v.eq_ignore_ascii_case("true") {
            vec![finding(self.id(), Severity::Warning, "git core.longpaths 未开启", desc).with_fix(fix_safety, fix_explain, fix_commands, fix_rollback)]
        } else { vec![] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_triggers_and_silent() {
        let mut env = crate::test_utils::empty_env();
        crate::test_utils::with_detector(&mut env, "toolchain.git", Layer::Toolchain,
            BTreeMap::from([("longpaths".to_string(), crate::test_utils::s("true"))]));
        assert_eq!(GitLongpathsDisabled.evaluate(&env).len(), 0);

        let mut env2 = crate::test_utils::empty_env();
        crate::test_utils::with_detector(&mut env2, "toolchain.git", Layer::Toolchain,
            BTreeMap::from([("longpaths".to_string(), crate::test_utils::s("false"))]));
        assert_eq!(GitLongpathsDisabled.evaluate(&env2).len(), 1);
    }
}
