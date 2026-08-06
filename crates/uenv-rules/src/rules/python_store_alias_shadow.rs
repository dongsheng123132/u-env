// python.store-alias-shadow — 规则。
// severity: Severity::Warning

#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use uenv_core::{DetectStatus, Environment, FactValue, Finding, Layer, Safety, Severity};

use crate::helpers::{finding, FindingExt};
use crate::Rule;

pub struct PythonStoreAliasShadow;

impl Rule for PythonStoreAliasShadow {
    fn id(&self) -> &'static str {
        "python.store-alias-shadow"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["toolchain.python"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        let desc = "PATH 里的 python.exe 命中了 %LOCALAPPDATA%\\Microsoft\\WindowsApps 下的 Store 重定向stub——它只是指向应用商店的占位符。你敲 python 要么弹商店要么报「应用未安装」，而真正的 Python（如 3.12）在 PATH 更后面的位置。装真 Python 或把 Python 目录挪到WindowsApps 前面。";
        let fix_safety = Safety::Confirm;
        let fix_explain = "把真实 Python 目录移到 PATH 前面，或禁用 Store 别名";
        let fix_commands: &[&str] = &["echo \"在 设置→应用→高级应用设置→应用执行别名 中关闭 python.exe 别名\""];
        let fix_rollback: &[&str] = &["（重新开启别名）"];
        let v = crate::helpers::fact_bool(env, "toolchain.python", "store_alias_shadow");
        let Some(true) = v else { return vec![]; };
        vec![finding(self.id(), Severity::Warning, "PATH 里命中 Microsoft Store 的 python 别名", desc).with_fix(fix_safety, fix_explain, fix_commands, fix_rollback)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_triggers_and_silent() {
        let mut env = crate::test_utils::empty_env();
        crate::test_utils::with_detector(&mut env, "toolchain.python", Layer::Toolchain,
            BTreeMap::from([("store_alias_shadow".to_string(), crate::test_utils::b(true))]));
        assert_eq!(PythonStoreAliasShadow.evaluate(&env).len(), 1);

        let mut env2 = crate::test_utils::empty_env();
        crate::test_utils::with_detector(&mut env2, "toolchain.python", Layer::Toolchain,
            BTreeMap::from([("store_alias_shadow".to_string(), crate::test_utils::b(false))]));
        assert_eq!(PythonStoreAliasShadow.evaluate(&env2).len(), 0);
    }
}
