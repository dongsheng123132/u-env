// host.disk detector — 各盘剩余空间与总容量。
// layer=Host
// 数据源：powershell Get-Volume | ConvertTo-Json
// ⚠️ 规格 §4 硬规则 5：free_bytes 是波动值 → 放 volatile，不进 facts。

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, EvidenceKind, FactValue, Layer};

use crate::context::{evidence_from_command, ScanContext};
use crate::detector::{Detector, DetectorMeta, DetectorResult};

pub struct HostDisk;

impl Detector for HostDisk {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "host.disk",
            layer: Layer::Host,
            title: "磁盘卷信息",
            cost: Cost::Slow,
        }
    }

    fn detect(&self, ctx: &ScanContext) -> DetectorResult {
        let out = ctx.run(
            "powershell",
            &[
                "-NoProfile",
                "-Command",
                "Get-Volume | Where-Object { $_.DriveLetter } | Select-Object DriveLetter,FileSystem,Size,SizeRemaining | ConvertTo-Json -Compress",
            ],
        );
        let mut evidence = Vec::new();
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "powershell Get-Volume | ConvertTo-Json",
            &out,
        ));

        let (facts, volatile) = if out.ran && out.exit_code == Some(0) {
            parse_volumes(&out.stdout)
        } else {
            (BTreeMap::new(), BTreeMap::new())
        };

        let volume_count = facts
            .get("volumes")
            .map(|v| match v {
                FactValue::Set(s) => s.len(),
                _ => 0,
            })
            .unwrap_or(0);

        let (status, summary) = if volume_count > 0 {
            (
                DetectStatus::Ok,
                format!("{} 个卷", volume_count),
            )
        } else if out.ran {
            (
                DetectStatus::Degraded,
                "Get-Volume 无输出或解析失败".to_string(),
            )
        } else {
            (
                DetectStatus::Error,
                "PowerShell 不可用".to_string(),
            )
        };

        DetectorResult {
            status,
            summary,
            facts,
            volatile,
            evidence,
        }
    }
}

/// 解析 Get-Volume JSON —— 与 IO 分离，独立可测。
/// facts: volumes = Set of Map { drive, filesystem, total_bytes }
/// volatile: free_bytes = Map { drive -> Int }
pub fn parse_volumes(
    json: &str,
) -> (BTreeMap<String, FactValue>, BTreeMap<String, FactValue>) {
    let mut facts = BTreeMap::new();
    let mut volatile = BTreeMap::new();

    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return (facts, volatile),
    };

    let arr = match v {
        serde_json::Value::Array(arr) => arr,
        serde_json::Value::Object(obj) => vec![serde_json::Value::Object(obj)],
        _ => return (facts, volatile),
    };

    let mut volumes: Vec<FactValue> = Vec::new();
    let mut free_map: BTreeMap<String, FactValue> = BTreeMap::new();

    for item in arr {
        let drive = item
            .get("DriveLetter")
            .and_then(|d| d.as_str())
            .filter(|d| !d.is_empty())
            .map(|d| format!("{d}:"));
        let fs = item
            .get("FileSystem")
            .and_then(|f| f.as_str())
            .filter(|f| !f.is_empty())
            .map(|f| f.to_string());
        let total = item
            .get("Size")
            .and_then(|s| s.as_u64())
            .map(|s| s as i64);
        let free = item
            .get("SizeRemaining")
            .and_then(|s| s.as_u64())
            .map(|s| s as i64);

        if let Some(drive) = drive {
            let mut map = BTreeMap::new();
            map.insert("drive".to_string(), FactValue::Str(drive.clone()));
            if let Some(fs) = fs {
                map.insert("filesystem".to_string(), FactValue::Str(fs));
            }
            if let Some(total) = total {
                map.insert("total_bytes".to_string(), FactValue::Int(total));
            }
            volumes.push(FactValue::Map(map));

            if let Some(free) = free {
                free_map.insert(drive, FactValue::Int(free));
            }
        }
    }

    if !volumes.is_empty() {
        facts.insert("volumes".to_string(), FactValue::Set(volumes));
    }
    if !free_map.is_empty() {
        volatile.insert("free_bytes".to_string(), FactValue::Map(free_map));
    }

    (facts, volatile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_multiple_volumes() {
        let json = r#"[
            {"DriveLetter":"C","FileSystem":"NTFS","Size":511659282432,"SizeRemaining":30853988352},
            {"DriveLetter":"D","FileSystem":"NTFS","Size":460387061760,"SizeRemaining":88562774016}
        ]"#;
        let (facts, volatile) = parse_volumes(json);

        let volumes = match facts.get("volumes").unwrap() {
            FactValue::Set(s) => s,
            _ => panic!("volumes should be Set"),
        };
        assert_eq!(volumes.len(), 2);

        // 第一个卷
        match volumes.first().unwrap() {
            FactValue::Map(m) => {
                assert_eq!(
                    m.get("drive").unwrap(),
                    &FactValue::Str("C:".to_string())
                );
                assert_eq!(
                    m.get("filesystem").unwrap(),
                    &FactValue::Str("NTFS".to_string())
                );
                assert_eq!(
                    m.get("total_bytes").unwrap(),
                    &FactValue::Int(511659282432)
                );
                // free_bytes 绝不在 facts 里
                assert!(!m.contains_key("free_bytes"));
            }
            _ => panic!("volume should be Map"),
        }

        // free_bytes 在 volatile
        let free = match volatile.get("free_bytes").unwrap() {
            FactValue::Map(m) => m,
            _ => panic!("free_bytes should be Map"),
        };
        assert_eq!(
            free.get("C:").unwrap(),
            &FactValue::Int(30853988352)
        );
    }

    #[test]
    fn parse_single_object() {
        // ConvertTo-Json 单个对象时输出 Object 而不是数组
        let json = r#"{"DriveLetter":"C","FileSystem":"NTFS","Size":100,"SizeRemaining":50}"#;
        let (facts, volatile) = parse_volumes(json);
        let volumes = match facts.get("volumes").unwrap() {
            FactValue::Set(s) => s,
            _ => panic!("volumes should be Set"),
        };
        assert_eq!(volumes.len(), 1);
        assert_eq!(volatile.get("free_bytes").is_some(), true);
    }

    #[test]
    fn parse_garbage_input() {
        // 畸形输入：不 panic，空结果
        let (facts, volatile) = parse_volumes("not json at all");
        assert!(facts.is_empty());
        assert!(volatile.is_empty());
    }

    #[test]
    fn parse_missing_fields() {
        // 缺 DriveLetter 的卷（系统保留卷）跳过
        let json = r#"[{"FileSystem":"NTFS","Size":100,"SizeRemaining":10}]"#;
        let (facts, _) = parse_volumes(json);
        assert!(facts.is_empty());
    }
}
