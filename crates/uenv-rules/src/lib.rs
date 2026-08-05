// uenv-rules — 规则引擎。T0 仅建 crate + 占位 trait，完整实现在 T5。
use uenv_core::{Environment, Finding};

/// 规则只读 Environment，不许跑命令、不许读注册表。
pub trait Rule: Send + Sync {
    fn id(&self) -> &'static str;
    fn evaluate(&self, env: &Environment) -> Vec<Finding>;
}

pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![]
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
