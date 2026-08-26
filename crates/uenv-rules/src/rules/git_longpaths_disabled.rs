// git.longpaths-disabled — 规则。
// severity: Severity::Warning

#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use uenv_core::{DetectStatus, Environment, FactValue, Finding, Layer, Safety, Severity};

use crate::Rule;
use crate::helpers::{FindingExt, finding};

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
        let fix_explain = "开启 git 长路径支持（写入 --global 配置；rollback 用 --unset 删除该键——本条只在键原本不存在时触发，unset 恰好还原原状；若你此前手工设过别的值，请以 git config --global core.longpaths 的旧值为准）";
        // 触发条件钉住了旧值：本规则仅在 longpaths 未配置（或非 true）时触发。
        // 「未配置」分支的 rollback（--unset）严格还原原状；「显式 false」分支
        // 的真逆操作是设回 false，见下方按触发原因选择 rollback。
        let fix_commands: &[&str] = &["git config --global core.longpaths true"];
        let fix_rollback_unset: &[&str] = &["git config --global --unset core.longpaths"];
        let fix_rollback_false: &[&str] = &["git config --global core.longpaths false"];
        let v = crate::helpers::fact_str(env, "toolchain.git", "longpaths");
        let Some(v) = v else {
            // 未配置默认关 → 触发（保守：宁报勿漏）
            return vec![
                finding(
                    self.id(),
                    Severity::Warning,
                    "git core.longpaths 未开启",
                    desc,
                )
                .with_executable_fix(
                    fix_safety,
                    fix_explain,
                    fix_commands,
                    fix_rollback_unset,
                ),
            ];
        };
        if !v.eq_ignore_ascii_case("true") {
            vec![
                finding(
                    self.id(),
                    Severity::Warning,
                    "git core.longpaths 未开启",
                    desc,
                )
                .with_executable_fix(
                    fix_safety,
                    fix_explain,
                    fix_commands,
                    fix_rollback_false,
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
            "toolchain.git",
            Layer::Toolchain,
            BTreeMap::from([("longpaths".to_string(), crate::test_utils::s("true"))]),
        );
        assert_eq!(GitLongpathsDisabled.evaluate(&env).len(), 0);

        let mut env2 = crate::test_utils::empty_env();
        crate::test_utils::with_detector(
            &mut env2,
            "toolchain.git",
            Layer::Toolchain,
            BTreeMap::from([("longpaths".to_string(), crate::test_utils::s("false"))]),
        );
        assert_eq!(GitLongpathsDisabled.evaluate(&env2).len(), 1);
    }
}
