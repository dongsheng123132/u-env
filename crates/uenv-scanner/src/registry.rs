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
        Box::new(detectors::windows_locale::WindowsLocale),
        Box::new(detectors::wsl_status::WslStatus),
        Box::new(detectors::fs_project_location::FsProjectLocation),
        Box::new(detectors::security_defender::SecurityDefender),
        Box::new(detectors::net_proxy::NetProxy),
        Box::new(detectors::host_disk::HostDisk),
        Box::new(detectors::host_hardware::HostHardware),
        Box::new(detectors::path_analysis::PathAnalysis),
        Box::new(detectors::toolchain_git::ToolchainGit),
        Box::new(detectors::toolchain_npm_family::ToolchainNpmFamily),
        Box::new(detectors::toolchain_python::ToolchainPython),
        Box::new(detectors::toolchain_rust::ToolchainRust),
        Box::new(detectors::toolchain_dotnet::ToolchainDotnet),
        Box::new(detectors::toolchain_msvc::ToolchainMsvc),
        Box::new(detectors::toolchain_windows_sdk::ToolchainWindowsSdk),
        Box::new(detectors::runtime_webview2::RuntimeWebView2),
        Box::new(detectors::project_kind::ProjectKind),
        Box::new(detectors::project_manifests::ProjectManifests),
        Box::new(detectors::project_lockfiles::ProjectLockfiles),
        Box::new(detectors::project_git::ProjectGit),
        Box::new(detectors::project_drift::ProjectDrift),
    ]
}
