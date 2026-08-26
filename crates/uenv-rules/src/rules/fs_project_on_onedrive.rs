// fs.project-on-onedrive — 规则。
// severity: Severity::Error

#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use uenv_core::{DetectStatus, Environment, FactValue, Finding, Layer, Safety, Severity};

use crate::Rule;
use crate::helpers::{FindingExt, finding};

pub struct FsProjectOnOnedrive;

impl Rule for FsProjectOnOnedrive {
    fn id(&self) -> &'static str {
        "fs.project-on-onedrive"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["fs.project-location"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        let desc = "项目放在 OneDrive 同步目录下。OneDrive 的按需文件（placeholder）会让编译器读到空壳文件，git 仓库的 .git 被同步还可能造成索引损坏；文件锁和同步冲突会让 node_modules 这类海量小文件项目慢到怀疑人生。把开发项目移到本地盘（C:\\dev 或 D:\\dev），OneDrive 只留文档。";
        let fix_safety = Safety::Manual;
        let fix_explain = "把项目移到 OneDrive 之外的本地目录并重新 clone（手动迁移，无法自动执行，也无法自动回滚）";
        // 手动档不提供命令；commands 为空则 rollback 允许为空（契约见架构文档 §3 SuggestedFix）
        let on = crate::helpers::fact_bool(env, "fs.project-location", "on_onedrive");
        let Some(true) = on else {
            return vec![];
        };
        vec![
            finding(self.id(), Severity::Error, "项目在 OneDrive 目录里", desc)
                .with_fix(fix_safety, fix_explain),
        ]
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
            BTreeMap::from([("on_onedrive".to_string(), crate::test_utils::b(true))]),
        );
        assert_eq!(FsProjectOnOnedrive.evaluate(&env).len(), 1);

        let mut env2 = crate::test_utils::empty_env();
        crate::test_utils::with_detector(
            &mut env2,
            "fs.project-location",
            Layer::Host,
            BTreeMap::from([("on_onedrive".to_string(), crate::test_utils::b(false))]),
        );
        assert_eq!(FsProjectOnOnedrive.evaluate(&env2).len(), 0);
    }
}
