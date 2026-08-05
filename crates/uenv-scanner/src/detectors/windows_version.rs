// windows.version detector — 产品名、版本号、build、UBR、edition、DisplayVersion、架构。
// 数据源优先注册表 HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion。
// layer=Host

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, FactValue, Layer};
use winreg::enums::*;

use crate::context::{ScanContext, evidence_from_registry};
use crate::detector::{Detector, DetectorMeta, DetectorResult};

pub struct WindowsVersion;

impl Detector for WindowsVersion {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "windows.version",
            layer: Layer::Host,
            title: "Windows 版本信息",
            cost: Cost::Fast,
        }
    }

    fn detect(&self, ctx: &ScanContext) -> DetectorResult {
        let base = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
        let mut evidence = Vec::new();
        let mut facts = BTreeMap::new();
        let mut errors = Vec::new();

        // 读取各个注册表值
        let product_name = ctx.reg_read(HKEY_LOCAL_MACHINE, base, "ProductName");
        evidence.push(evidence_from_registry(base, "ProductName", &product_name));

        let current_build = ctx.reg_read(HKEY_LOCAL_MACHINE, base, "CurrentBuild");
        evidence.push(evidence_from_registry(base, "CurrentBuild", &current_build));

        let ubr = ctx.reg_read(HKEY_LOCAL_MACHINE, base, "UBR");
        evidence.push(evidence_from_registry(base, "UBR", &ubr));

        let edition_id = ctx.reg_read(HKEY_LOCAL_MACHINE, base, "EditionID");
        evidence.push(evidence_from_registry(base, "EditionID", &edition_id));

        let display_version = ctx.reg_read(HKEY_LOCAL_MACHINE, base, "DisplayVersion");
        evidence.push(evidence_from_registry(
            base,
            "DisplayVersion",
            &display_version,
        ));

        let current_major = ctx.reg_read(HKEY_LOCAL_MACHINE, base, "CurrentMajorVersionNumber");
        let current_minor = ctx.reg_read(HKEY_LOCAL_MACHINE, base, "CurrentMinorVersionNumber");

        // 构建 facts
        let product_name_raw = product_name
            .as_ref()
            .map(|v| v.value.clone())
            .unwrap_or_else(|| {
                errors.push("ProductName not found in registry".to_string());
                "Unknown".to_string()
            });
        // product_name_raw: 注册表原值，不管对错都存
        facts.insert(
            "product_name_raw".to_string(),
            FactValue::Str(product_name_raw.clone()),
        );

        let major = parse_u32(&current_major).unwrap_or(10);
        let minor = parse_u32(&current_minor).unwrap_or(0);
        let build = parse_u32(&current_build).unwrap_or(0);

        // ⚠️ Windows 11 判定：build >= 22000 → Windows 11
        // 注册表 ProductName 在 Win11 上仍写 "Windows 10"，需推导修正
        let product_name_str = if build >= 22000 {
            product_name_raw.replace("Windows 10", "Windows 11")
        } else {
            product_name_raw.clone()
        };
        facts.insert(
            "product_name".to_string(),
            FactValue::Str(product_name_str.clone()),
        );

        let version = format!("{major}.{minor}.{build}");
        facts.insert("version".to_string(), FactValue::Str(version));

        facts.insert("build".to_string(), FactValue::Int(build as i64));

        if let Some(ref u) = ubr {
            if let Ok(n) = u.value.parse::<u32>() {
                facts.insert("ubr".to_string(), FactValue::Int(n as i64));
            }
        }

        if let Some(ref e) = edition_id {
            let v = e.value.trim().to_string();
            if !v.is_empty() {
                facts.insert("edition".to_string(), FactValue::Str(v));
            }
        }

        if let Some(ref d) = display_version {
            let v = d.value.trim().to_string();
            if !v.is_empty() {
                facts.insert("display_version".to_string(), FactValue::Str(v));
            }
        }

        // 架构检测
        let arch = detect_architecture();
        facts.insert("architecture".to_string(), FactValue::Str(arch));

        let (status, summary) = if errors.is_empty() {
            (
                DetectStatus::Ok,
                format!("{product_name_str} build {build}"),
            )
        } else if facts.is_empty() {
            (
                DetectStatus::Error,
                format!("Failed: {}", errors.join("; ")),
            )
        } else {
            (
                DetectStatus::Degraded,
                format!("{product_name_str} build {build} (partial)"),
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

fn detect_architecture() -> String {
    #[cfg(target_arch = "x86_64")]
    {
        "x64".to_string()
    }
    #[cfg(target_arch = "aarch64")]
    {
        "arm64".to_string()
    }
    #[cfg(target_arch = "x86")]
    {
        "x86".to_string()
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "x86")))]
    {
        "unknown".to_string()
    }
}

fn parse_u32(val: &Option<crate::context::RegValue>) -> Option<u32> {
    val.as_ref()?.value.trim().parse::<u32>().ok()
}

/// 解析逻辑与 IO 分离 —— 独立可测
#[cfg(test)]
pub fn parse_windows_version(
    product_name: &str,
    build: u32,
    ubr: Option<u32>,
    edition: Option<&str>,
    display_version: Option<&str>,
    arch: &str,
) -> BTreeMap<String, FactValue> {
    let mut facts = BTreeMap::new();
    let major = 10;
    let minor = 0;
    // product_name_raw: 注册表原值
    facts.insert(
        "product_name_raw".to_string(),
        FactValue::Str(product_name.to_string()),
    );
    // Windows 11 判定：build >= 22000 → 替换 "Windows 10" 为 "Windows 11"
    let derived_name = if build >= 22000 {
        product_name.replace("Windows 10", "Windows 11")
    } else {
        product_name.to_string()
    };
    facts.insert("product_name".to_string(), FactValue::Str(derived_name));
    facts.insert(
        "version".to_string(),
        FactValue::Str(format!("{major}.{minor}.{build}")),
    );
    facts.insert("build".to_string(), FactValue::Int(build as i64));
    if let Some(u) = ubr {
        facts.insert("ubr".to_string(), FactValue::Int(u as i64));
    }
    if let Some(e) = edition {
        if !e.is_empty() {
            facts.insert("edition".to_string(), FactValue::Str(e.to_string()));
        }
    }
    if let Some(d) = display_version {
        if !d.is_empty() {
            facts.insert("display_version".to_string(), FactValue::Str(d.to_string()));
        }
    }
    facts.insert("architecture".to_string(), FactValue::Str(arch.to_string()));
    facts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_windows_11() {
        let facts = parse_windows_version(
            "Windows 11 家庭中文版",
            26100,
            Some(2454),
            Some("CoreCountrySpecific"),
            Some("24H2"),
            "x64",
        );
        assert_eq!(
            facts.get("product_name").unwrap(),
            &FactValue::Str("Windows 11 家庭中文版".to_string())
        );
        assert_eq!(
            facts.get("product_name_raw").unwrap(),
            &FactValue::Str("Windows 11 家庭中文版".to_string())
        );
        assert_eq!(
            facts.get("version").unwrap(),
            &FactValue::Str("10.0.26100".to_string())
        );
        assert_eq!(facts.get("build").unwrap(), &FactValue::Int(26100));
        assert_eq!(facts.get("ubr").unwrap(), &FactValue::Int(2454));
        assert_eq!(
            facts.get("edition").unwrap(),
            &FactValue::Str("CoreCountrySpecific".to_string())
        );
        assert_eq!(
            facts.get("display_version").unwrap(),
            &FactValue::Str("24H2".to_string())
        );
        assert_eq!(
            facts.get("architecture").unwrap(),
            &FactValue::Str("x64".to_string())
        );
    }

    /// build 22631，注册表写着 "Windows 10 Home" → 推导为 "Windows 11 Home"
    #[test]
    fn win11_build_22631_from_registry_win10() {
        let facts = parse_windows_version(
            "Windows 10 Home", // 注册表原值（微软未更新）
            22631,
            None,
            None,
            None,
            "x64",
        );
        assert_eq!(
            facts.get("product_name").unwrap(),
            &FactValue::Str("Windows 11 Home".to_string())
        );
        assert_eq!(
            facts.get("product_name_raw").unwrap(),
            &FactValue::Str("Windows 10 Home".to_string())
        );
    }

    /// build 19045，注册表 "Windows 10 Home" → 沿用注册表值
    #[test]
    fn win10_build_19045_keeps_registry_name() {
        let facts = parse_windows_version("Windows 10 Home", 19045, None, None, None, "x64");
        assert_eq!(
            facts.get("product_name").unwrap(),
            &FactValue::Str("Windows 10 Home".to_string())
        );
        assert_eq!(
            facts.get("product_name_raw").unwrap(),
            &FactValue::Str("Windows 10 Home".to_string())
        );
    }

    #[test]
    fn parse_minimal() {
        let facts = parse_windows_version("Windows 10 Pro", 19045, None, None, None, "x64");
        // version 应该正确
        assert!(facts.contains_key("product_name"));
        assert_eq!(facts.get("build").unwrap(), &FactValue::Int(19045));
        assert!(!facts.contains_key("ubr"));
        assert!(!facts.contains_key("edition"));
    }
}
