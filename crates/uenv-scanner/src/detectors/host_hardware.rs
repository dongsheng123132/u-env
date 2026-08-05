// host.hardware detector — CPU 型号/核数、内存总量、是否虚拟机。
// layer=Host
// 数据源：Get-CimInstance Win32_Processor / Win32_ComputerSystem
// is_virtual 启发式：Manufacturer/Model 匹配已知虚拟化特征。

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, EvidenceKind, FactValue, Layer};

use crate::context::{evidence_from_command, ScanContext};
use crate::detector::{Detector, DetectorMeta, DetectorResult};

pub struct HostHardware;

impl Detector for HostHardware {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "host.hardware",
            layer: Layer::Host,
            title: "硬件信息",
            cost: Cost::Slow,
        }
    }

    fn detect(&self, ctx: &ScanContext) -> DetectorResult {
        // CPU + 内存/厂商/型号一次拿齐（一条 PowerShell 拼 JSON，减少子进程开销）
        let ps_cmd = "powershell -NoProfile -Command \"$cpu=Get-CimInstance Win32_Processor | Select-Object -First 1; $cs=Get-CimInstance Win32_ComputerSystem; [PSCustomObject]@{cpu_name=$cpu.Name; cores=$cpu.NumberOfCores; ram=$cs.TotalPhysicalMemory; manufacturer=$cs.Manufacturer; model=$cs.Model} | ConvertTo-Json -Compress\"";
        let out = ctx.run(
            "powershell",
            &[
                "-NoProfile",
                "-Command",
                "$cpu=Get-CimInstance Win32_Processor | Select-Object -First 1; $cs=Get-CimInstance Win32_ComputerSystem; [PSCustomObject]@{cpu_name=$cpu.Name; cores=$cpu.NumberOfCores; ram=$cs.TotalPhysicalMemory; manufacturer=$cs.Manufacturer; model=$cs.Model} | ConvertTo-Json -Compress",
            ],
        );
        let mut evidence = Vec::new();
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            ps_cmd,
            &out,
        ));

        let mut facts = BTreeMap::new();
        let summary: String;
        let status = if out.ran && out.exit_code == Some(0) {
            let parsed = parse_hardware(&out.stdout);
            if parsed.is_empty() {
                summary = "CIM 查询输出解析失败".to_string();
                DetectStatus::Degraded
            } else {
                summary = parsed
                    .get("cpu_model")
                    .map(|v| match v {
                        FactValue::Str(s) => s.clone(),
                        _ => String::new(),
                    })
                    .unwrap_or_default();
                facts = parsed;
                DetectStatus::Ok
            }
        } else {
            summary = "PowerShell 不可用".to_string();
            DetectStatus::Error
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

/// 虚拟化特征关键字（Manufacturer/Model 大小写不敏感匹配）
const VIRTUALIZATION_HINTS: &[&str] = &[
    "vmware",
    "virtualbox",
    "innotek",
    "qemu",
    "kvm",
    "xen",
    "hyper-v",
    "bochs",
    "parallels",
    "microsoft corporation virtual",
    "virtual machine",
    "hvm",
    "bhyve",
];

/// 解析 CIM JSON —— 与 IO 分离，独立可测。
/// 输入形如 {"cpu_name":"...","cores":14,"ram":68501889024,"manufacturer":"LENOVO","model":"82RF"}
pub fn parse_hardware(json: &str) -> BTreeMap<String, FactValue> {
    let mut facts = BTreeMap::new();

    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return facts,
    };

    let obj = match v {
        serde_json::Value::Object(obj) => obj,
        serde_json::Value::Array(arr) => match arr.first() {
            Some(serde_json::Value::Object(o)) => o.clone(),
            _ => return facts,
        },
        _ => return facts,
    };

    if let Some(name) = obj.get("cpu_name").and_then(|n| n.as_str()) {
        if !name.is_empty() {
            facts.insert("cpu_model".to_string(), FactValue::Str(name.to_string()));
        }
    }
    if let Some(cores) = obj.get("cores").and_then(|c| c.as_u64()) {
        facts.insert("cpu_cores".to_string(), FactValue::Int(cores as i64));
    }
    if let Some(ram) = obj.get("ram").and_then(|r| r.as_u64()) {
        facts.insert("ram_total_bytes".to_string(), FactValue::Int(ram as i64));
    }

    let manufacturer = obj
        .get("manufacturer")
        .and_then(|m| m.as_str())
        .unwrap_or("")
        .to_lowercase();
    let model = obj
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("")
        .to_lowercase();
    let combined = format!("{manufacturer} {model}");

    let is_virtual = VIRTUALIZATION_HINTS
        .iter()
        .any(|hint| combined.contains(hint));
    facts.insert("is_virtual".to_string(), FactValue::Bool(is_virtual));

    facts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_physical_machine() {
        // 本机实测输出
        let json = r#"{"cpu_name":"12th Gen Intel(R) Core(TM) i9-12900H","cores":14,"ram":68501889024,"manufacturer":"LENOVO","model":"82RF"}"#;
        let facts = parse_hardware(json);
        assert_eq!(
            facts.get("cpu_model").unwrap(),
            &FactValue::Str("12th Gen Intel(R) Core(TM) i9-12900H".to_string())
        );
        assert_eq!(facts.get("cpu_cores").unwrap(), &FactValue::Int(14));
        assert_eq!(
            facts.get("ram_total_bytes").unwrap(),
            &FactValue::Int(68501889024)
        );
        assert_eq!(facts.get("is_virtual").unwrap(), &FactValue::Bool(false));
    }

    #[test]
    fn parse_vmware_vm() {
        let json = r#"{"cpu_name":"Intel(R) Xeon(R) Gold 6248R","cores":4,"ram":8589934592,"manufacturer":"VMware, Inc.","model":"VMware Virtual Platform"}"#;
        let facts = parse_hardware(json);
        assert_eq!(facts.get("is_virtual").unwrap(), &FactValue::Bool(true));
    }

    #[test]
    fn parse_hyperv_vm() {
        let json = r#"{"cpu_name":"Intel(R) Core(TM)","cores":2,"ram":4294967296,"manufacturer":"Microsoft Corporation","model":"Virtual Machine"}"#;
        let facts = parse_hardware(json);
        assert_eq!(facts.get("is_virtual").unwrap(), &FactValue::Bool(true));
    }

    #[test]
    fn parse_garbage_input() {
        let facts = parse_hardware("not json");
        assert!(facts.is_empty());
    }
}
