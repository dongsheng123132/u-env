// webview2.missing — 规则。
// severity: Severity::Error

#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use uenv_core::{DetectStatus, Environment, FactValue, Finding, Layer, Safety, Severity};

use crate::Rule;
use crate::helpers::{FindingExt, finding};

pub struct Webview2Missing;

impl Rule for Webview2Missing {
    fn id(&self) -> &'static str {
        "webview2.missing"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["runtime.webview2", "project.kind"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        let desc = "Tauri/Electron 项目在 Windows 上依赖 WebView2 Runtime（Evergreen）。未安装时应用启动直接白屏或报「找不到 WebView2」。Win10 较新版本自带，但 Win10 旧版和精简版需要手动装 Evergreen Runtime。";
        let fix_safety = Safety::Manual;
        let fix_explain = "下载并安装 WebView2 Evergreen Runtime（https://developer.microsoft.com/microsoft-edge/webview2/；安装走系统安装器，卸载 Runtime 即回滚）";
        let installed = crate::helpers::fact_bool(env, "runtime.webview2", "installed");
        let Some(false) = installed else {
            return vec![];
        };
        if crate::helpers::kind_is(env, "tauri") || crate::helpers::kind_is(env, "electron") {
            vec![
                finding(self.id(), Severity::Error, "WebView2 Runtime 未安装", desc)
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
        crate::test_utils::with_detector(
            &mut env,
            "runtime.webview2",
            Layer::Toolchain,
            BTreeMap::from([("installed".to_string(), crate::test_utils::b(false))]),
        );
        crate::test_utils::with_project_kinds(&mut env, &["tauri"]);
        assert_eq!(Webview2Missing.evaluate(&env).len(), 1);

        let mut env2 = crate::test_utils::empty_env();
        crate::test_utils::with_detector(
            &mut env2,
            "runtime.webview2",
            Layer::Toolchain,
            BTreeMap::from([("installed".to_string(), crate::test_utils::b(true))]),
        );
        crate::test_utils::with_project_kinds(&mut env2, &["tauri"]);
        assert_eq!(Webview2Missing.evaluate(&env2).len(), 0);
    }
}
