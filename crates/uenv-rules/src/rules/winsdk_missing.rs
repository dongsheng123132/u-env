// winsdk.missing — 规则。
// severity: Severity::Error

#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use uenv_core::{DetectStatus, Environment, FactValue, Finding, Layer, Safety, Severity};

use crate::helpers::{finding, FindingExt};
use crate::Rule;

pub struct WinsdkMissing;

impl Rule for WinsdkMissing {
    fn id(&self) -> &'static str {
        "winsdk.missing"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["toolchain.windows-sdk", "project.kind"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        let desc = "Tauri/WinUi 项目链接时需要 Windows SDK（windows crate / C++ / WinAppSDK 都要）。SDK 缺失时链接报一堆 unresolved external symbol 或找不到 windows.h。随 Build Tools 一起装 Windows SDK 组件即可。";
        let fix_safety = Safety::Manual;
        let fix_explain = "在 VS Installer 里勾选 Windows SDK（或装独立 Windows SDK）";
        let fix_commands: &[&str] = &["start https://developer.microsoft.com/windows/downloads/windows-sdk/"];
        let fix_rollback: &[&str] = &["（卸载 SDK）"];
        if !crate::helpers::kind_is(env, "tauri") && !crate::helpers::kind_is(env, "winui") {
            return vec![];
        }
        let has_versions = crate::helpers::fact_collection_len(env, "toolchain.windows-sdk", "versions");
        match has_versions {
            Some(n) if n > 0 => vec![],
            _ => vec![finding(self.id(), Severity::Error, "Windows SDK 未安装", desc).with_fix(fix_safety, fix_explain, fix_commands, fix_rollback)],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_triggers_and_silent() {
        let mut env = crate::test_utils::empty_env();
        crate::test_utils::with_project_kinds(&mut env, &["tauri"]);
        crate::test_utils::with_detector(&mut env, "toolchain.windows-sdk", Layer::Toolchain,
            BTreeMap::from([("versions".to_string(), crate::test_utils::set_str(&[]))]));
        assert_eq!(WinsdkMissing.evaluate(&env).len(), 1);

        let mut env2 = crate::test_utils::empty_env();
        crate::test_utils::with_project_kinds(&mut env2, &["tauri"]);
        crate::test_utils::with_detector(&mut env2, "toolchain.windows-sdk", Layer::Toolchain,
            BTreeMap::from([("versions".to_string(), crate::test_utils::set_str(&["10.0.19041.0"]))]));
        assert_eq!(WinsdkMissing.evaluate(&env2).len(), 0);
    }
}
