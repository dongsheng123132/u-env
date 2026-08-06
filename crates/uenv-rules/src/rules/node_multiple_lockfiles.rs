// node.multiple-lockfiles — 规则。
// severity: Severity::Warning

#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use uenv_core::{DetectStatus, Environment, FactValue, Finding, Layer, Safety, Severity};

use crate::helpers::{finding, FindingExt};
use crate::Rule;

pub struct NodeMultipleLockfiles;

impl Rule for NodeMultipleLockfiles {
    fn id(&self) -> &'static str {
        "node.multiple-lockfiles"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["project.lockfiles"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        let desc = "同一项目同时存在 ≥2 种锁文件（package-lock.json + pnpm-lock.yaml + yarn.lock 混放）。CI 和队友各用各的锁文件时，依赖树会漂移，今天锁得住明天锁不住，是「在我机器上是好的」的高发来源。只保留你实际使用的管理器对应的那一份。";
        let fix_safety = Safety::Manual;
        let fix_explain = "删除不用的锁文件，只留当前包管理器对应的一份";
        let fix_commands: &[&str] = &["echo \"例如保留 pnpm-lock.yaml，删除 package-lock.json 和 yarn.lock\""];
        let fix_rollback: &[&str] = &["git checkout -- . 2>NUL"];
        let n = crate::helpers::lockfile_names(env).len();
        if n >= 2 {
            vec![finding(self.id(), Severity::Warning, "存在多种锁文件", desc).with_fix(fix_safety, fix_explain, fix_commands, fix_rollback)]
        } else { vec![] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_triggers_and_silent() {
        let mut env = crate::test_utils::empty_env();
        crate::test_utils::with_detector(&mut env, "project.lockfiles", Layer::Project,
            BTreeMap::from([("lockfiles".to_string(), FactValue::Map(BTreeMap::from([
                ("package-lock.json".to_string(), crate::test_utils::s("a")),
                ("pnpm-lock.yaml".to_string(), crate::test_utils::s("b")),
            ])))]));
        assert_eq!(NodeMultipleLockfiles.evaluate(&env).len(), 1);

        let mut env2 = crate::test_utils::empty_env();
        crate::test_utils::with_detector(&mut env2, "project.lockfiles", Layer::Project,
            BTreeMap::from([("lockfiles".to_string(), FactValue::Map(BTreeMap::from([
                ("pnpm-lock.yaml".to_string(), crate::test_utils::s("b")),
            ])))]));
        assert_eq!(NodeMultipleLockfiles.evaluate(&env2).len(), 0);
    }
}
