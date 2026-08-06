// windows.developer-mode-disabled — 规则。
// severity: Severity::Warning

#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use uenv_core::{DetectStatus, Environment, FactValue, Finding, Layer, Safety, Severity};

use crate::helpers::{finding, FindingExt};
use crate::Rule;

pub struct WindowsDeveloperModeDisabled;

impl Rule for WindowsDeveloperModeDisabled {
    fn id(&self) -> &'static str {
        "windows.developer-mode-disabled"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["windows.developer-mode", "project.kind"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        let desc = "开发者模式关闭时，符号链接创建（mklink）、部分调试器附加、UWP 侧载会被限制。Tauri/WinUi 项目的调试流程（尤其 symlink 依赖和开发证书）可能莫名失败。这个开关只影响开发体验，不影响已构建产物的运行。";
        let fix_safety = Safety::Confirm;
        let fix_explain = "开启开发者模式（设置 → 隐私和安全性 → 开发者选项）";
        let fix_commands: &[&str] = &["start ms-settings:developers"];
        let fix_rollback: &[&str] = &["（手动关闭同一开关）"];
        let enabled = crate::helpers::fact_bool(env, "windows.developer-mode", "enabled");
        let Some(false) = enabled else { return vec![]; };
        if crate::helpers::kind_is(env, "tauri") || crate::helpers::kind_is(env, "winui") {
            vec![finding(self.id(), Severity::Warning, "开发者模式未开启", desc).with_fix(fix_safety, fix_explain, fix_commands, fix_rollback)]
        } else { vec![] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_triggers_and_silent() {
        let mut env = crate::test_utils::empty_env();
        crate::test_utils::with_detector(&mut env, "windows.developer-mode", Layer::Host,
            BTreeMap::from([("enabled".to_string(), crate::test_utils::b(false))]));
        crate::test_utils::with_project_kinds(&mut env, &["tauri", "rust"]);
        assert_eq!(WindowsDeveloperModeDisabled.evaluate(&env).len(), 1);

        // 非 tauri/winui 项目 → 不触发
        let mut env2 = crate::test_utils::empty_env();
        crate::test_utils::with_detector(&mut env2, "windows.developer-mode", Layer::Host,
            BTreeMap::from([("enabled".to_string(), crate::test_utils::b(false))]));
        crate::test_utils::with_project_kinds(&mut env2, &["node"]);
        assert_eq!(WindowsDeveloperModeDisabled.evaluate(&env2).len(), 0);
    }
}
