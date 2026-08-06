// rust.gnu-toolchain-on-windows — 规则。
// severity: Severity::Warning

#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use uenv_core::{DetectStatus, Environment, FactValue, Finding, Layer, Safety, Severity};

use crate::Rule;
use crate::helpers::{FindingExt, finding};

pub struct RustGnuToolchainOnWindows;

impl Rule for RustGnuToolchainOnWindows {
    fn id(&self) -> &'static str {
        "rust.gnu-toolchain-on-windows"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["toolchain.rust", "project.kind"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        let desc = "active toolchain 是 *-pc-windows-gnu。Tauri 依赖 MSVC 链接的 WebView2 绑定，GNU toolchain 下要么编不过要么运行时崩溃。Tauri 项目在 Windows 必须用 MSVC toolchain。rustup default stable-x86_64-pc-windows-msvc 即可。";
        let fix_safety = Safety::Confirm;
        let fix_explain = "切换到 MSVC toolchain";
        let fix_commands: &[&str] = &["rustup default stable-x86_64-pc-windows-msvc"];
        let fix_rollback: &[&str] = &["rustup default stable-x86_64-pc-windows-gnu"];
        if !crate::helpers::kind_is(env, "tauri") {
            return vec![];
        }
        let active = crate::helpers::fact_str(env, "toolchain.rust", "active_toolchain");
        let Some(active) = active else {
            return vec![];
        };
        if active.contains("-gnu") {
            vec![
                finding(
                    self.id(),
                    Severity::Warning,
                    "Windows 上使用 GNU toolchain 构建 Tauri",
                    desc,
                )
                .with_fix(fix_safety, fix_explain, fix_commands, fix_rollback),
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
        crate::test_utils::with_project_kinds(&mut env, &["tauri"]);
        crate::test_utils::with_detector(
            &mut env,
            "toolchain.rust",
            Layer::Toolchain,
            BTreeMap::from([(
                "active_toolchain".to_string(),
                crate::test_utils::s("stable-x86_64-pc-windows-gnu"),
            )]),
        );
        assert_eq!(RustGnuToolchainOnWindows.evaluate(&env).len(), 1);

        let mut env2 = crate::test_utils::empty_env();
        crate::test_utils::with_project_kinds(&mut env2, &["tauri"]);
        crate::test_utils::with_detector(
            &mut env2,
            "toolchain.rust",
            Layer::Toolchain,
            BTreeMap::from([(
                "active_toolchain".to_string(),
                crate::test_utils::s("stable-x86_64-pc-windows-msvc"),
            )]),
        );
        assert_eq!(RustGnuToolchainOnWindows.evaluate(&env2).len(), 0);
    }
}
