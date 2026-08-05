use crate::detector::Detector;
use crate::detectors;

/// 显式注册所有 detector。新 detector 在这里加一行。
/// 不用 inventory/linkme —— 显式列表更好 review。
pub fn all_detectors() -> Vec<Box<dyn Detector>> {
    vec![
        Box::new(detectors::windows_version::WindowsVersion),
        Box::new(detectors::toolchain_node::ToolchainNode),
        Box::new(detectors::windows_powershell::WindowsPowerShell),
        Box::new(detectors::windows_developer_mode::WindowsDeveloperMode),
        Box::new(detectors::windows_long_paths::WindowsLongPaths),
    ]
}
