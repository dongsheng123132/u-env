// windows.long-paths-disabled — 规则。
// severity: Severity::Warning

#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use uenv_core::{DetectStatus, Environment, FactValue, Finding, Layer, Safety, Severity};

use crate::Rule;
use crate::helpers::{FindingExt, finding};

pub struct WindowsLongPathsDisabled;

impl Rule for WindowsLongPathsDisabled {
    fn id(&self) -> &'static str {
        "windows.long-paths-disabled"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["windows.long-paths", "project.kind", "fs.project-location"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        let desc = "Windows 默认 PATH_MAX 260 字符。node_modules 深嵌套 + 中文/长项目名很容易超限，报错长相各异（ENAMETOOLONG、\"系统找不到指定的路径\"、npm 装一半失败）。项目路径越深或依赖越重（Node 项目几乎必中）风险越高。开启后新进程生效，已开的终端要重开。";
        let fix_safety = Safety::Confirm;
        let fix_explain = "开启系统长路径支持（需管理员），改注册表后重启终端";
        let fix_commands: &[&str] = &[
            "powershell -NoProfile -Command \"New-ItemProperty -Path 'HKLM:\\SYSTEM\\CurrentControlSet\\Control\\FileSystem' -Name LongPathsEnabled -Value 1 -PropertyType DWord -Force\"",
        ];
        let fix_rollback: &[&str] = &[
            "powershell -NoProfile -Command \"Set-ItemProperty -Path 'HKLM:\\SYSTEM\\CurrentControlSet\\Control\\FileSystem' -Name LongPathsEnabled -Value 0\"",
        ];
        let enabled = crate::helpers::fact_bool(env, "windows.long-paths", "enabled");
        let Some(false) = enabled else {
            return vec![];
        };
        // Node 项目 → Error；否则 Warning（任务书：项目路径深或 kind 含 Node 时 Error）
        let is_node = crate::helpers::kind_is(env, "node");
        let sev = if is_node {
            Severity::Error
        } else {
            Severity::Warning
        };
        vec![finding(self.id(), sev, "长路径支持未开启", desc).with_fix(
            fix_safety,
            fix_explain,
            fix_commands,
            fix_rollback,
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_triggers_and_silent() {
        // 未开启 + Node 项目 → Error
        let mut env = crate::test_utils::empty_env();
        crate::test_utils::with_detector(
            &mut env,
            "windows.long-paths",
            Layer::Host,
            BTreeMap::from([("enabled".to_string(), crate::test_utils::b(false))]),
        );
        crate::test_utils::with_project_kinds(&mut env, &["node"]);
        let out = WindowsLongPathsDisabled.evaluate(&env);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Error);

        // 已开启 → 不触发
        let mut env2 = crate::test_utils::empty_env();
        crate::test_utils::with_detector(
            &mut env2,
            "windows.long-paths",
            Layer::Host,
            BTreeMap::from([("enabled".to_string(), crate::test_utils::b(true))]),
        );
        assert_eq!(WindowsLongPathsDisabled.evaluate(&env2).len(), 0);
    }
}
