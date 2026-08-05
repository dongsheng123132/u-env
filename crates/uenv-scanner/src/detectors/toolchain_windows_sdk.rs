// toolchain.windows-sdk detector — 已安装的 Windows SDK 版本列表与根目录。
// layer=Toolchain
// 数据源：注册表 HKLM\SOFTWARE\WOW6432Node\Microsoft\Microsoft SDKs\Windows\v10.0
// 的 InstallationFolder，再列 <root>\Include 下子目录。

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, FactValue, Layer};
use winreg::enums::HKEY_LOCAL_MACHINE;

use crate::context::{evidence_from_registry, ScanContext};
use crate::detector::{Detector, DetectorMeta, DetectorResult};

pub struct ToolchainWindowsSdk;

impl Detector for ToolchainWindowsSdk {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "toolchain.windows-sdk",
            layer: Layer::Toolchain,
            title: "Windows SDK",
            cost: Cost::Fast,
        }
    }

    fn detect(&self, ctx: &ScanContext) -> DetectorResult {
        let path = r"SOFTWARE\WOW6432Node\Microsoft\Microsoft SDKs\Windows\v10.0";
        let name = "InstallationFolder";

        let value = ctx.reg_read(HKEY_LOCAL_MACHINE, path, name);
        let mut evidence = Vec::new();
        evidence.push(evidence_from_registry(path, name, &value));

        let mut facts = BTreeMap::new();
        let mut sdk_root = String::new();
        let mut versions: Vec<FactValue> = Vec::new();

        if let Some(v) = &value {
            let root = v.value.trim_end_matches(['\\', '/']).to_string();
            if !root.is_empty() {
                sdk_root = root.clone();
                facts.insert("sdk_root".to_string(), FactValue::Path(root.clone()));

                // 列 <root>\Include 下子目录 = 已装 SDK 版本
                let include_dir = format!("{root}\\Include");
                if let Ok(entries) = std::fs::read_dir(&include_dir) {
                    let mut dirs: Vec<String> = entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().is_dir())
                        .map(|e| e.file_name().to_string_lossy().to_string())
                        .collect();
                    dirs.sort();
                    versions = dirs.into_iter().map(FactValue::Str).collect();
                }
            }
        }

        if !versions.is_empty() {
            facts.insert("versions".to_string(), FactValue::Set(versions.clone()));
        }

        let (status, summary) = if sdk_root.is_empty() {
            (
                DetectStatus::Absent,
                "Windows SDK 未安装（注册表键缺失）".to_string(),
            )
        } else {
            (
                DetectStatus::Ok,
                if versions.is_empty() {
                    format!("SDK 根 {sdk_root}，未发现 Include 版本目录")
                } else {
                    format!(
                        "{} 个 SDK 版本，最新 {}",
                        versions.len(),
                        match versions.last().unwrap() {
                            FactValue::Str(s) => s.clone(),
                            _ => String::new(),
                        }
                    )
                },
            )
        };

        DetectorResult {
            status,
            summary,
            facts,
            volatile: BTreeMap::new(),
            evidence,
        }
    }
}

/// 解析逻辑与 IO 分离 —— 独立可测
pub fn parse_windows_sdk(
    installation_folder: Option<&str>,
    include_subdirs: &[&str],
) -> BTreeMap<String, FactValue> {
    let mut facts = BTreeMap::new();
    let root = installation_folder.unwrap_or("").trim_end_matches(['\\', '/']);
    if !root.is_empty() {
        facts.insert("sdk_root".to_string(), FactValue::Path(root.to_string()));
        let versions: Vec<FactValue> = include_subdirs
            .iter()
            .map(|s| FactValue::Str(s.to_string()))
            .collect();
        if !versions.is_empty() {
            facts.insert("versions".to_string(), FactValue::Set(versions));
        }
    }
    facts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_with_versions() {
        let facts = parse_windows_sdk(
            Some(r"C:\Program Files (x86)\Windows Kits\10\"),
            &["10.0.19041.0", "10.0.26100.0"],
        );
        assert_eq!(
            facts.get("sdk_root").unwrap(),
            &FactValue::Path(r"C:\Program Files (x86)\Windows Kits\10".to_string())
        );
        assert_eq!(
            facts.get("versions").unwrap(),
            &FactValue::Set(vec![
                FactValue::Str("10.0.19041.0".to_string()),
                FactValue::Str("10.0.26100.0".to_string()),
            ])
        );
    }

    #[test]
    fn parse_missing_key() {
        let facts = parse_windows_sdk(None, &[]);
        assert!(facts.is_empty());
    }

    #[test]
    fn parse_garbage_input() {
        // 畸形输入：不 panic
        let facts = parse_windows_sdk(Some(""), &[]);
        assert!(facts.is_empty());
    }
}
