// toolchain.msvc detector — Visual Studio / Build Tools 实例、C++ workload、MSVC 工具集版本。
// layer=Toolchain
// 数据源：vswhere.exe（固定路径，run_slow 20s）+ VC\Tools\MSVC 目录探测
// ⚠️ vswhere 的 packages 字段对旧版实例（如 VS2019 BuildTools）可能为空，
//    has_cpp_workload 以 VC\Tools\MSVC 目录存在为准（这是 C++ workload 的直接证据）。

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, EvidenceKind, FactValue, Layer};

use crate::context::{ScanContext, evidence_from_command};
use crate::detector::{Detector, DetectorMeta, DetectorResult};

const VSWHERE: &str = r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe";

pub struct ToolchainMsvc;

impl Detector for ToolchainMsvc {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "toolchain.msvc",
            layer: Layer::Toolchain,
            title: "MSVC 工具链",
            cost: Cost::Slow,
        }
    }

    fn detect(&self, ctx: &ScanContext) -> DetectorResult {
        let mut evidence = Vec::new();

        let vswhere = ctx.run_slow(VSWHERE, &["-products", "*", "-format", "json", "-utf8"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "vswhere -products * -format json -utf8",
            &vswhere,
        ));

        // vswhere 不存在 → 没装 VS/Build Tools → Absent
        if !vswhere.ran {
            return DetectorResult {
                status: DetectStatus::Absent,
                summary: "未安装 Visual Studio / Build Tools".to_string(),
                facts: BTreeMap::new(),
                volatile: BTreeMap::new(),
                evidence,
            };
        }

        let (instances, degraded) = parse_vswhere(&vswhere.stdout);

        // IO 层：对每个实例探测 VC\Tools\MSVC 目录（C++ workload 的直接证据）
        let mut instance_facts: Vec<FactValue> = Vec::new();
        let mut has_cpp = false;
        for mut inst in instances {
            let path = match inst.get("install_path") {
                Some(FactValue::Path(p)) => p.clone(),
                _ => String::new(),
            };
            if !path.is_empty() {
                let vc_msvc = format!("{path}\\VC\\Tools\\MSVC");
                if let Ok(entries) = std::fs::read_dir(&vc_msvc) {
                    has_cpp = true;
                    let versions: Vec<FactValue> = entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().is_dir())
                        .map(|e| FactValue::Version(e.file_name().to_string_lossy().to_string()))
                        .collect();
                    if !versions.is_empty() {
                        inst.insert(
                            "msvc_toolset_versions".to_string(),
                            FactValue::Set(versions),
                        );
                    }
                }
            }
            instance_facts.push(FactValue::Map(inst));
        }

        let mut facts = BTreeMap::new();
        if !instance_facts.is_empty() {
            facts.insert("instances".to_string(), FactValue::Set(instance_facts));
        }
        facts.insert("has_cpp_workload".to_string(), FactValue::Bool(has_cpp));

        let instance_count = facts
            .get("instances")
            .map(|v| match v {
                FactValue::Set(s) => s.len(),
                _ => 0,
            })
            .unwrap_or(0);

        let (status, summary) = if instance_count == 0 && degraded {
            (DetectStatus::Degraded, "vswhere 输出无法解析".to_string())
        } else if instance_count == 0 {
            (DetectStatus::Ok, "vswhere 可用但无实例".to_string())
        } else {
            (
                DetectStatus::Ok,
                format!(
                    "{} 个实例{}",
                    instance_count,
                    if has_cpp {
                        "（含 C++ workload）"
                    } else {
                        ""
                    }
                ),
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

/// 解析 vswhere JSON 数组 —— 纯字符串解析，与 IO 分离。
/// 返回 Vec<Map>（display_name/version/install_path），C++ workload 探测在 detect() 里做。
/// 返回 (instances, degraded)：degraded=true 表示 JSON 解析失败。
pub fn parse_vswhere(json: &str) -> (Vec<BTreeMap<String, FactValue>>, bool) {
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return (Vec::new(), true),
    };

    let arr = match v {
        serde_json::Value::Array(arr) => arr,
        _ => return (Vec::new(), true),
    };

    let mut instances: Vec<BTreeMap<String, FactValue>> = Vec::new();
    for inst in arr {
        let mut m = BTreeMap::new();
        if let Some(name) = inst.get("displayName").and_then(|n| n.as_str()) {
            m.insert("display_name".to_string(), FactValue::Str(name.to_string()));
        }
        if let Some(ver) = inst.get("installationVersion").and_then(|v| v.as_str()) {
            m.insert("version".to_string(), FactValue::Version(ver.to_string()));
        }
        if let Some(path) = inst.get("installationPath").and_then(|p| p.as_str()) {
            m.insert(
                "install_path".to_string(),
                FactValue::Path(path.to_string()),
            );
        }
        instances.push(m);
    }

    (instances, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_instance() {
        let json = r#"[{
            "instanceId": "167a8d60",
            "installationPath": "C:\\Program Files (x86)\\Microsoft Visual Studio\\2019\\BuildTools",
            "installationVersion": "16.11.35931.194",
            "displayName": "Visual Studio 生成工具 2019"
        }]"#;
        let (instances, degraded) = parse_vswhere(json);
        assert!(!degraded);
        assert_eq!(instances.len(), 1);
        let m = &instances[0];
        assert_eq!(
            m.get("display_name").unwrap(),
            &FactValue::Str("Visual Studio 生成工具 2019".to_string())
        );
        assert_eq!(
            m.get("version").unwrap(),
            &FactValue::Version("16.11.35931.194".to_string())
        );
        assert_eq!(
            m.get("install_path").unwrap(),
            &FactValue::Path(
                r"C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools".to_string()
            )
        );
    }

    #[test]
    fn parse_garbage_input() {
        let (instances, degraded) = parse_vswhere("not json");
        assert!(degraded);
        assert!(instances.is_empty());
    }

    #[test]
    fn parse_empty_array() {
        let (instances, degraded) = parse_vswhere("[]");
        assert!(!degraded);
        assert!(instances.is_empty());
    }
}
