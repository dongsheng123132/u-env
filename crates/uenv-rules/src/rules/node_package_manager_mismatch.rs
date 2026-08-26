// node.package-manager-mismatch — 规则。
// severity: Severity::Warning

#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use uenv_core::{DetectStatus, Environment, FactValue, Finding, Layer, Safety, Severity};

use crate::Rule;
use crate::helpers::{FindingExt, finding};

pub struct NodePackageManagerMismatch;

impl Rule for NodePackageManagerMismatch {
    fn id(&self) -> &'static str {
        "node.package-manager-mismatch"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["project.manifests", "project.lockfiles"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        let desc = "package.json 的 packageManager 声明了 pnpm/yarn，但项目里实际生成的是另一种锁文件（如声明 pnpm 却有 package-lock.json）。pnpm 的 node_modules 布局和 npm 完全不同，用错管理器装依赖会得到残缺的 node_modules，运行时各种 module not found。锁文件是谁生成的，就用谁安装——删掉多余的锁文件后统一用一种管理器。";
        let fix_safety = Safety::Manual;
        let fix_explain = "统一包管理器：保留 packageManager 声明的管理器对应的锁文件，删除其余后用声明的管理器重装（如声明 pnpm：删 package-lock.json 后跑 pnpm install；误删可 git checkout -- <文件名> 找回）";
        let pm = crate::helpers::fact_str(env, "project.manifests", "package_manager");
        let lockfiles = crate::helpers::lockfile_names(env);
        let Some(pm) = pm else {
            return vec![];
        };
        // 声明 pnpm → 期望 pnpm-lock.yaml；yarn → yarn.lock；npm → package-lock.json
        let expected = match pm.as_str() {
            "pnpm" => "pnpm-lock.yaml",
            "yarn" => "yarn.lock",
            "npm" => "package-lock.json",
            _ => return vec![],
        };
        if !lockfiles.is_empty() && !lockfiles.iter().any(|f| f == expected) {
            vec![
                finding(self.id(), Severity::Warning, "包管理器与锁文件不一致", desc)
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
        // 声明 pnpm 但有 package-lock.json → 触发
        let mut env = crate::test_utils::empty_env();
        crate::test_utils::with_detector(
            &mut env,
            "project.manifests",
            Layer::Project,
            BTreeMap::from([("package_manager".to_string(), crate::test_utils::s("pnpm"))]),
        );
        crate::test_utils::with_detector(
            &mut env,
            "project.lockfiles",
            Layer::Project,
            BTreeMap::from([(
                "lockfiles".to_string(),
                FactValue::Map(BTreeMap::from([(
                    "package-lock.json".to_string(),
                    crate::test_utils::s("abc"),
                )])),
            )]),
        );
        assert_eq!(NodePackageManagerMismatch.evaluate(&env).len(), 1);

        // 声明 pnpm 且有 pnpm-lock.yaml → 不触发
        let mut env2 = crate::test_utils::empty_env();
        crate::test_utils::with_detector(
            &mut env2,
            "project.manifests",
            Layer::Project,
            BTreeMap::from([("package_manager".to_string(), crate::test_utils::s("pnpm"))]),
        );
        crate::test_utils::with_detector(
            &mut env2,
            "project.lockfiles",
            Layer::Project,
            BTreeMap::from([(
                "lockfiles".to_string(),
                FactValue::Map(BTreeMap::from([(
                    "pnpm-lock.yaml".to_string(),
                    crate::test_utils::s("abc"),
                )])),
            )]),
        );
        assert_eq!(NodePackageManagerMismatch.evaluate(&env2).len(), 0);
    }
}
