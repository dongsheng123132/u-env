// uenv-rules — 规则引擎：把 scan 的事实变成诊断。
// 规则只读 Environment，绝不跑命令/读注册表/访问文件系统。

pub mod helpers;
pub mod rules;
#[cfg(test)]
pub mod test_utils;

use uenv_core::{Environment, Finding};

/// 规则只读 Environment，不许跑命令、不许读注册表。
pub trait Rule: Send + Sync {
    fn id(&self) -> &'static str;
    /// 本规则依赖哪些 detector 的 fact —— doctor 按 adapter 的 relevant_detectors 过滤用
    fn relevant_detectors(&self) -> &'static [&'static str];
    fn evaluate(&self, env: &Environment) -> Vec<Finding>;
}

/// 显式规则列表（同 detector registry 风格）
pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(rules::path_duplicate_entries::PathDuplicateEntries),
        Box::new(rules::path_missing_entries::PathMissingEntries),
        Box::new(rules::node_multiple_in_path::NodeMultipleInPath),
        Box::new(rules::rust_multiple_cargo_in_path::RustMultipleCargoInPath),
        Box::new(rules::node_version_drift::NodeVersionDrift),
        Box::new(rules::rust_version_drift::RustVersionDrift),
        Box::new(rules::node_package_manager_mismatch::NodePackageManagerMismatch),
        Box::new(rules::node_multiple_lockfiles::NodeMultipleLockfiles),
        Box::new(rules::windows_long_paths_disabled::WindowsLongPathsDisabled),
        Box::new(rules::windows_developer_mode_disabled::WindowsDeveloperModeDisabled),
        Box::new(rules::fs_project_on_onedrive::FsProjectOnOnedrive),
        Box::new(rules::fs_project_on_network::FsProjectOnNetwork),
        Box::new(rules::fs_project_path_non_ascii::FsProjectPathNonAscii),
        Box::new(rules::fs_project_path_has_space::FsProjectPathHasSpace),
        Box::new(rules::webview2_missing::Webview2Missing),
        Box::new(rules::msvc_missing_buildtools::MsvcMissingBuildtools),
        Box::new(rules::winsdk_missing::WinsdkMissing),
        Box::new(rules::rust_gnu_toolchain_on_windows::RustGnuToolchainOnWindows),
        Box::new(rules::git_autocrlf_true::GitAutocrlfTrue),
        Box::new(rules::git_longpaths_disabled::GitLongpathsDisabled),
        Box::new(rules::net_proxy_inconsistent::NetProxyInconsistent),
        Box::new(rules::security_defender_scans_project::SecurityDefenderScansProject),
        Box::new(rules::python_store_alias_shadow::PythonStoreAliasShadow),
        Box::new(rules::scan_detector_failed::ScanDetectorFailed),
    ]
}

/// 规则 id 列表（供 doctor 的 skipped 标记）
pub fn all_rule_ids() -> Vec<&'static str> {
    all_rules().iter().map(|r| r.id()).collect()
}

/// 关键 fact 键清单 —— uenv diff 的风险分级依据（§5.5）。
/// 格式："detector_id:fact_key"，"*" 表示该 detector 全部 fact 键都是关键。
/// 这里定义，diff 不自己猜。T5 规则引擎也复用这份清单。
pub fn critical_fact_keys() -> &'static [&'static str] {
    &[
        "toolchain.node:versions",
        "toolchain.node:executables",
        "toolchain.rust:active_toolchain",
        "toolchain.rust:cargo_paths",
        "toolchain.msvc:instances",
        "toolchain.windows-sdk:versions",
        "runtime.webview2:installed",
        "runtime.webview2:version",
        "windows.long-paths:enabled",
        "windows.developer-mode:enabled",
        "fs.project-location:on_onedrive",
        "fs.project-location:on_network",
        "path.analysis:shadowed_exes",
        "project.lockfiles:*",
        "project.drift:*",
    ]
}

/// 判断某 detector 的某 fact 键是否关键
pub fn is_critical_fact(detector_id: &str, key: &str) -> bool {
    let detector_wildcard = format!("{detector_id}:*");
    critical_fact_keys()
        .iter()
        .any(|k| *k == format!("{detector_id}:{key}") || *k == detector_wildcard)
}
