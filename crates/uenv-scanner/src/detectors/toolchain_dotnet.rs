// toolchain.dotnet detector — dotnet SDK / runtime / WindowsDesktop runtime 列表。
// layer=Toolchain
// 数据源：dotnet --list-sdks / dotnet --list-runtimes（慢，20s 超时）
// 未安装 → Absent，不是 Error。

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, EvidenceKind, FactValue, Layer};

use crate::context::{evidence_from_command, ScanContext};
use crate::detector::{Detector, DetectorMeta, DetectorResult};

pub struct ToolchainDotnet;

impl Detector for ToolchainDotnet {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "toolchain.dotnet",
            layer: Layer::Toolchain,
            title: ".NET SDK",
            cost: Cost::Slow,
        }
    }

    fn detect(&self, ctx: &ScanContext) -> DetectorResult {
        let mut evidence = Vec::new();

        let sdks = ctx.run_slow("dotnet", &["--list-sdks"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "dotnet --list-sdks",
            &sdks,
        ));

        // 未安装 → Absent
        if !sdks.ran {
            return DetectorResult {
                status: DetectStatus::Absent,
                summary: ".NET SDK 未安装（dotnet 不在 PATH）".to_string(),
                facts: BTreeMap::new(),
                volatile: BTreeMap::new(),
                evidence,
            };
        }

        let runtimes = ctx.run_slow("dotnet", &["--list-runtimes"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "dotnet --list-runtimes",
            &runtimes,
        ));

        let facts = parse_dotnet(&sdks.stdout, &runtimes.stdout);

        let sdk_count = facts
            .get("sdk_versions")
            .map(|v| match v {
                FactValue::Set(s) => s.len(),
                _ => 0,
            })
            .unwrap_or(0);

        let (status, summary) = if sdk_count > 0 {
            (
                DetectStatus::Ok,
                format!("{sdk_count} 个 SDK"),
            )
        } else {
            (
                DetectStatus::Error,
                "dotnet --list-sdks 无输出或解析失败".to_string(),
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

/// 解析 dotnet 输出 —— 与 IO 分离，独立可测。
/// sdks: "10.0.203 [C:\Program Files\dotnet\sdk]"
/// runtimes: "Microsoft.NETCore.App 8.0.14 [C:\...]" / "Microsoft.WindowsDesktop.App 8.0.14 [...]"
pub fn parse_dotnet(sdks_out: &str, runtimes_out: &str) -> BTreeMap<String, FactValue> {
    let mut facts = BTreeMap::new();

    // SDK 版本
    let mut sdk_versions: Vec<FactValue> = Vec::new();
    for line in sdks_out.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // 版本号是第一个空格前
        let version = line.split_whitespace().next().unwrap_or("");
        if !version.is_empty() && version.contains('.') {
            sdk_versions.push(FactValue::Version(version.to_string()));
        }
    }
    if !sdk_versions.is_empty() {
        facts.insert("sdk_versions".to_string(), FactValue::Set(sdk_versions));
    }

    // Runtime 版本
    let mut runtime_versions: Vec<FactValue> = Vec::new();
    let mut windows_desktop: Vec<FactValue> = Vec::new();
    for line in runtimes_out.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let name = parts.next().unwrap_or("");
        let version = parts.next().unwrap_or("");
        if name.is_empty() || version.is_empty() {
            continue;
        }
        if name.starts_with("Microsoft.WindowsDesktop.App") {
            windows_desktop.push(FactValue::Version(version.to_string()));
        } else {
            runtime_versions.push(FactValue::Version(version.to_string()));
        }
    }
    if !runtime_versions.is_empty() {
        facts.insert("runtime_versions".to_string(), FactValue::Set(runtime_versions));
    }
    if !windows_desktop.is_empty() {
        facts.insert(
            "windows_desktop_runtimes".to_string(),
            FactValue::Set(windows_desktop),
        );
    }

    facts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_dotnet() {
        let sdks = "10.0.203 [C:\\Program Files\\dotnet\\sdk]\r\n";
        let runtimes = "Microsoft.AspNetCore.App 10.0.7 [C:\\Program Files\\dotnet\\shared\\Microsoft.AspNetCore.App]\r\n\
                        Microsoft.NETCore.App 8.0.14 [C:\\Program Files\\dotnet\\shared\\Microsoft.NETCore.App]\r\n\
                        Microsoft.WindowsDesktop.App 8.0.14 [C:\\Program Files\\dotnet\\shared\\Microsoft.WindowsDesktop.App]\r\n";
        let facts = parse_dotnet(sdks, runtimes);
        assert_eq!(
            facts.get("sdk_versions").unwrap(),
            &FactValue::Set(vec![FactValue::Version("10.0.203".to_string())])
        );
        assert_eq!(
            facts.get("runtime_versions").unwrap(),
            &FactValue::Set(vec![
                FactValue::Version("10.0.7".to_string()),
                FactValue::Version("8.0.14".to_string()),
            ])
        );
        assert_eq!(
            facts.get("windows_desktop_runtimes").unwrap(),
            &FactValue::Set(vec![FactValue::Version("8.0.14".to_string())])
        );
    }

    #[test]
    fn parse_garbage_input() {
        let facts = parse_dotnet("!!!\n", "???\n");
        assert!(facts.is_empty());
    }

    #[test]
    fn parse_empty() {
        let facts = parse_dotnet("", "");
        assert!(facts.is_empty());
    }
}
