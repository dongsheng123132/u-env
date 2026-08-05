// windows.powershell detector — Windows PowerShell 5.x 与 PowerShell 7+ (pwsh) 的版本与路径。
// layer=Host
// 数据源：powershell -NoProfile -Command $PSVersionTable.PSVersion.ToString() / pwsh 同款。

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, EvidenceKind, FactValue, Layer};

use crate::context::{ScanContext, evidence_from_command};
use crate::detector::{Detector, DetectorMeta, DetectorResult};

pub struct WindowsPowerShell;

impl Detector for WindowsPowerShell {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "windows.powershell",
            layer: Layer::Host,
            title: "PowerShell 版本",
            cost: Cost::Slow,
        }
    }

    fn detect(&self, ctx: &ScanContext) -> DetectorResult {
        let mut evidence = Vec::new();
        let mut facts = BTreeMap::new();

        // Windows PowerShell 5.x（系统自带 powershell.exe）
        let wps_cmd = "powershell -NoProfile -Command $PSVersionTable.PSVersion.ToString()";
        let wps = ctx.run(
            "powershell",
            &[
                "-NoProfile",
                "-Command",
                "$PSVersionTable.PSVersion.ToString()",
            ],
        );
        evidence.push(evidence_from_command(EvidenceKind::Command, wps_cmd, &wps));

        let wps_version = parse_version_output(&wps);

        // PowerShell 7+（pwsh，可能未安装）
        let pwsh_cmd = "pwsh -NoProfile -Command $PSVersionTable.PSVersion.ToString()";
        let pwsh = ctx.run(
            "pwsh",
            &[
                "-NoProfile",
                "-Command",
                "$PSVersionTable.PSVersion.ToString()",
            ],
        );
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            pwsh_cmd,
            &pwsh,
        ));

        let pwsh_version = parse_version_output(&pwsh);

        // pwsh 安装路径（which_all 已脱敏）
        let pwsh_path = if pwsh_version.is_some() {
            ctx.which_all("pwsh")
                .first()
                .map(|p| p.to_string_lossy().to_string())
        } else {
            None
        };

        if let Some(v) = &wps_version {
            facts.insert("wps_version".to_string(), FactValue::Version(v.clone()));
        }
        if let Some(v) = &pwsh_version {
            facts.insert("pwsh_version".to_string(), FactValue::Version(v.clone()));
        }
        if let Some(p) = &pwsh_path {
            facts.insert("pwsh_path".to_string(), FactValue::Path(p.clone()));
        }

        let (status, summary) = match (&wps_version, &pwsh_version) {
            (Some(w), Some(p)) => (
                DetectStatus::Ok,
                format!("Windows PowerShell {w} / PowerShell 7+ {p}"),
            ),
            (Some(w), None) => (
                DetectStatus::Ok,
                format!("Windows PowerShell {w}（未安装 PowerShell 7+）"),
            ),
            (None, Some(p)) => (
                DetectStatus::Ok,
                format!("未找到 Windows PowerShell，PowerShell 7+ {p}"),
            ),
            (None, None) => (
                DetectStatus::Error,
                "两个 PowerShell 都拿不到版本".to_string(),
            ),
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

/// 从 CommandOutcome 提取版本号：trim + 去尾部 CR/LF。失败返回 None。
fn parse_version_output(out: &crate::context::CommandOutcome) -> Option<String> {
    if !out.ran || out.exit_code != Some(0) {
        return None;
    }
    let v = out.stdout.trim();
    if v.is_empty() {
        None
    } else {
        Some(v.to_string())
    }
}

/// 解析逻辑与 IO 分离 —— 独立可测
#[cfg(test)]
pub fn parse_powershell(
    wps_version: Option<&str>,
    pwsh_version: Option<&str>,
    pwsh_path: Option<&str>,
) -> BTreeMap<String, FactValue> {
    let mut facts = BTreeMap::new();
    if let Some(v) = wps_version {
        facts.insert("wps_version".to_string(), FactValue::Version(v.to_string()));
    }
    if let Some(v) = pwsh_version {
        facts.insert(
            "pwsh_version".to_string(),
            FactValue::Version(v.to_string()),
        );
    }
    if let Some(p) = pwsh_path {
        facts.insert("pwsh_path".to_string(), FactValue::Path(p.to_string()));
    }
    facts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_both_installed() {
        let facts = parse_powershell(
            Some("5.1.22621.6133"),
            Some("7.6.4"),
            Some(r"C:\Program Files\PowerShell\7\pwsh.exe"),
        );
        assert_eq!(
            facts.get("wps_version").unwrap(),
            &FactValue::Version("5.1.22621.6133".to_string())
        );
        assert_eq!(
            facts.get("pwsh_version").unwrap(),
            &FactValue::Version("7.6.4".to_string())
        );
        assert_eq!(
            facts.get("pwsh_path").unwrap(),
            &FactValue::Path(r"C:\Program Files\PowerShell\7\pwsh.exe".to_string())
        );
    }

    #[test]
    fn parse_wps_only() {
        let facts = parse_powershell(Some("5.1.22621.6133"), None, None);
        assert!(facts.contains_key("wps_version"));
        assert!(!facts.contains_key("pwsh_version"));
        assert!(!facts.contains_key("pwsh_path"));
    }

    #[test]
    fn parse_both_missing() {
        let facts = parse_powershell(None, None, None);
        assert!(facts.is_empty());
    }
}
