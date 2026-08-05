// windows.long-paths detector — 长路径支持（LongPathsEnabled）是否开启。
// layer=Host
// 数据源：HKLM\SYSTEM\CurrentControlSet\Control\FileSystem\LongPathsEnabled

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, FactValue, Layer};
use winreg::enums::HKEY_LOCAL_MACHINE;

use crate::context::{evidence_from_registry, ScanContext};
use crate::detector::{Detector, DetectorMeta, DetectorResult};

pub struct WindowsLongPaths;

impl Detector for WindowsLongPaths {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "windows.long-paths",
            layer: Layer::Host,
            title: "长路径支持",
            cost: Cost::Fast,
        }
    }

    fn detect(&self, ctx: &ScanContext) -> DetectorResult {
        let path = r"SYSTEM\CurrentControlSet\Control\FileSystem";
        let name = "LongPathsEnabled";

        let value = ctx.reg_read(HKEY_LOCAL_MACHINE, path, name);
        let mut evidence = Vec::new();
        evidence.push(evidence_from_registry(path, name, &value));

        let mut facts = BTreeMap::new();
        let enabled = parse_dword_bool(&value);
        facts.insert("enabled".to_string(), FactValue::Bool(enabled));

        let (status, summary) = if value.is_some() {
            (
                DetectStatus::Ok,
                if enabled {
                    "长路径支持已开启".to_string()
                } else {
                    "长路径支持未开启".to_string()
                },
            )
        } else {
            (
                DetectStatus::Ok,
                "注册表键缺失，视为未开启".to_string(),
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

/// 注册表 DWORD 字符串 → bool：非零 true，解析失败 false
fn parse_dword_bool(value: &Option<crate::context::RegValue>) -> bool {
    match value {
        Some(v) => v.value.trim().parse::<i64>().unwrap_or(0) != 0,
        None => false,
    }
}

/// 解析逻辑与 IO 分离 —— 独立可测
#[cfg(test)]
pub fn parse_long_paths(reg_value: Option<&str>) -> BTreeMap<String, FactValue> {
    let mut facts = BTreeMap::new();
    let enabled = match reg_value {
        Some(v) => v.trim().parse::<i64>().unwrap_or(0) != 0,
        None => false,
    };
    facts.insert("enabled".to_string(), FactValue::Bool(enabled));
    facts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_enabled() {
        let facts = parse_long_paths(Some("1"));
        assert_eq!(facts.get("enabled").unwrap(), &FactValue::Bool(true));
    }

    #[test]
    fn parse_disabled() {
        let facts = parse_long_paths(Some("0"));
        assert_eq!(facts.get("enabled").unwrap(), &FactValue::Bool(false));
    }

    #[test]
    fn parse_missing_key() {
        let facts = parse_long_paths(None);
        assert_eq!(facts.get("enabled").unwrap(), &FactValue::Bool(false));
    }

    #[test]
    fn parse_garbage_input() {
        // 畸形输入：解析失败 → false，不 panic
        let facts = parse_long_paths(Some("abc"));
        assert_eq!(facts.get("enabled").unwrap(), &FactValue::Bool(false));
    }
}
