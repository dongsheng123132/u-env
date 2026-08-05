// windows.locale detector — 系统区域、ANSI 代码页、UTF-8 beta 开关。
// layer=Host
// 数据源：注册表 HKLM\SYSTEM\CurrentControlSet\Control\Nls\CodePage (ACP/OEMCP)、
//         cmd /c chcp（佐证）、(Get-WinSystemLocale).Name

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, EvidenceKind, FactValue, Layer};
use winreg::enums::HKEY_LOCAL_MACHINE;

use crate::context::{evidence_from_command, evidence_from_registry, ScanContext};
use crate::detector::{Detector, DetectorMeta, DetectorResult};

pub struct WindowsLocale;

impl Detector for WindowsLocale {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "windows.locale",
            layer: Layer::Host,
            title: "系统区域与代码页",
            cost: Cost::Slow,
        }
    }

    fn detect(&self, ctx: &ScanContext) -> DetectorResult {
        let path = r"SYSTEM\CurrentControlSet\Control\Nls\CodePage";
        let mut evidence = Vec::new();

        let acp = ctx.reg_read(HKEY_LOCAL_MACHINE, path, "ACP");
        evidence.push(evidence_from_registry(path, "ACP", &acp));
        let oem_cp = ctx.reg_read(HKEY_LOCAL_MACHINE, path, "OEMCP");
        evidence.push(evidence_from_registry(path, "OEMCP", &oem_cp));

        // chcp 佐证（cmd 内建，git-bash 里没有 chcp 可执行文件）
        let chcp = ctx.run("cmd", &["/c", "chcp"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "cmd /c chcp",
            &chcp,
        ));

        // 系统区域（非 Unicode 程序语言）
        let locale = ctx.run(
            "powershell",
            &["-NoProfile", "-Command", "(Get-WinSystemLocale).Name"],
        );
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "powershell -NoProfile -Command (Get-WinSystemLocale).Name",
            &locale,
        ));

        let acp_s = acp.as_ref().map(|v| v.value.trim().to_string());
        let oem_s = oem_cp.as_ref().map(|v| v.value.trim().to_string());
        let locale_s = if locale.ran && locale.exit_code == Some(0) {
            let t = locale.stdout.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        } else {
            None
        };

        let facts = parse_locale(acp_s.as_deref(), oem_s.as_deref(), locale_s.as_deref());

        let (status, summary) = if acp_s.is_some() {
            (
                DetectStatus::Ok,
                format!(
                    "ACP {} / OEM {} / locale {}",
                    acp_s.as_deref().unwrap_or("?"),
                    oem_s.as_deref().unwrap_or("?"),
                    locale_s.as_deref().unwrap_or("?")
                ),
            )
        } else {
            (
                DetectStatus::Error,
                "CodePage 注册表键读取失败".to_string(),
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
pub fn parse_locale(
    acp: Option<&str>,
    oem_cp: Option<&str>,
    system_locale: Option<&str>,
) -> BTreeMap<String, FactValue> {
    let mut facts = BTreeMap::new();
    if let Some(a) = acp {
        if !a.is_empty() {
            facts.insert("acp".to_string(), FactValue::Str(a.to_string()));
            // UTF-8 beta 开关：ACP == 65001
            facts.insert(
                "utf8_beta".to_string(),
                FactValue::Bool(a == "65001"),
            );
        }
    }
    if let Some(o) = oem_cp {
        if !o.is_empty() {
            facts.insert("oem_cp".to_string(), FactValue::Str(o.to_string()));
        }
    }
    if let Some(l) = system_locale {
        if !l.is_empty() {
            facts.insert("system_locale".to_string(), FactValue::Str(l.to_string()));
        }
    }
    facts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_zh_cn_gbk() {
        // 中文系统：ACP=936, OEMCP=936, locale=zh-CN，UTF-8 beta 关
        let facts = parse_locale(Some("936"), Some("936"), Some("zh-CN"));
        assert_eq!(facts.get("acp").unwrap(), &FactValue::Str("936".to_string()));
        assert_eq!(
            facts.get("oem_cp").unwrap(),
            &FactValue::Str("936".to_string())
        );
        assert_eq!(
            facts.get("system_locale").unwrap(),
            &FactValue::Str("zh-CN".to_string())
        );
        assert_eq!(facts.get("utf8_beta").unwrap(), &FactValue::Bool(false));
    }

    #[test]
    fn parse_utf8_beta_on() {
        // UTF-8 beta 开启：ACP=65001
        let facts = parse_locale(Some("65001"), Some("65001"), Some("en-US"));
        assert_eq!(facts.get("utf8_beta").unwrap(), &FactValue::Bool(true));
    }

    #[test]
    fn parse_missing_registry() {
        // 注册表读不到 → 无 acp，不 panic
        let facts = parse_locale(None, None, None);
        assert!(facts.is_empty());
    }
}
