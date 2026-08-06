// msvc.missing-buildtools — 规则。
// severity: Severity::Error

#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use uenv_core::{DetectStatus, Environment, FactValue, Finding, Layer, Safety, Severity};

use crate::Rule;
use crate::helpers::{FindingExt, finding};

pub struct MsvcMissingBuildtools;

impl Rule for MsvcMissingBuildtools {
    fn id(&self) -> &'static str {
        "msvc.missing-buildtools"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["toolchain.msvc", "project.kind"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        let desc = "Rust(Tauri)/C++ 项目需要 MSVC 工具链（Build Tools 或 VS 含 C++ workload）。缺失时 rustc 报 linker not found（link.exe），cargo build 一步都走不了。只装 rustup 不装 MSVC 是 Windows 新手最常见的坑——rustc 装好了但没 linker。";
        let fix_safety = Safety::Manual;
        let fix_explain = "安装 Visual Studio Build Tools，勾选「使用 C++ 的桌面开发」workload";
        let fix_commands: &[&str] = &[
            "start https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022",
        ];
        let fix_rollback: &[&str] = &["（卸载 Build Tools）"];
        if !crate::helpers::kind_is(env, "rust") && !crate::helpers::kind_is(env, "tauri") {
            return vec![];
        }
        let has_cpp = crate::helpers::fact_bool(env, "toolchain.msvc", "has_cpp_workload");
        match has_cpp {
            Some(true) => vec![],
            _ => vec![
                finding(self.id(), Severity::Error, "缺少 MSVC 构建工具", desc).with_fix(
                    fix_safety,
                    fix_explain,
                    fix_commands,
                    fix_rollback,
                ),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_triggers_and_silent() {
        let mut env = crate::test_utils::empty_env();
        crate::test_utils::with_project_kinds(&mut env, &["rust"]);
        crate::test_utils::with_detector(
            &mut env,
            "toolchain.msvc",
            Layer::Toolchain,
            BTreeMap::from([("has_cpp_workload".to_string(), crate::test_utils::b(false))]),
        );
        assert_eq!(MsvcMissingBuildtools.evaluate(&env).len(), 1);

        let mut env2 = crate::test_utils::empty_env();
        crate::test_utils::with_project_kinds(&mut env2, &["rust"]);
        crate::test_utils::with_detector(
            &mut env2,
            "toolchain.msvc",
            Layer::Toolchain,
            BTreeMap::from([("has_cpp_workload".to_string(), crate::test_utils::b(true))]),
        );
        assert_eq!(MsvcMissingBuildtools.evaluate(&env2).len(), 0);
    }
}
