// runtime.webview2 detector — WebView2 Runtime 是否安装、版本、渠道（Evergreen/Fixed）。
// layer=Toolchain
// 数据源：注册表 HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-...} 的 pv，
//         HKCU 同路径也要查（用户级安装）。

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, FactValue, Layer};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

use crate::context::{evidence_from_registry, ScanContext};
use crate::detector::{Detector, DetectorMeta, DetectorResult};

const WEBVIEW2_CLIENT: &str =
    r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
const WEBVIEW2_CLIENT_HKCU: &str =
    r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";

pub struct RuntimeWebView2;

impl Detector for RuntimeWebView2 {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "runtime.webview2",
            layer: Layer::Toolchain,
            title: "WebView2 Runtime",
            cost: Cost::Fast,
        }
    }

    fn detect(&self, ctx: &ScanContext) -> DetectorResult {
        let mut evidence = Vec::new();

        // HKLM（机器级安装）→ HKCU（用户级安装）依次查
        let hklm_pv = ctx.reg_read(HKEY_LOCAL_MACHINE, WEBVIEW2_CLIENT, "pv");
        evidence.push(evidence_from_registry(WEBVIEW2_CLIENT, "pv", &hklm_pv));
        let hklm_channel = ctx.reg_read(HKEY_LOCAL_MACHINE, WEBVIEW2_CLIENT, "channel");
        evidence.push(evidence_from_registry(WEBVIEW2_CLIENT, "channel", &hklm_channel));

        let hkcu_pv = ctx.reg_read(HKEY_CURRENT_USER, WEBVIEW2_CLIENT_HKCU, "pv");
        evidence.push(evidence_from_registry(WEBVIEW2_CLIENT_HKCU, "pv", &hkcu_pv));
        let hkcu_channel = ctx.reg_read(HKEY_CURRENT_USER, WEBVIEW2_CLIENT_HKCU, "channel");
        evidence.push(evidence_from_registry(WEBVIEW2_CLIENT_HKCU, "channel", &hkcu_channel));

        // 优先机器级，其次用户级
        let pv = hklm_pv.as_ref().or(hkcu_pv.as_ref());
        let channel = hklm_channel.as_ref().or(hkcu_channel.as_ref());

        let mut facts = BTreeMap::new();
        let installed = pv.is_some();
        facts.insert("installed".to_string(), FactValue::Bool(installed));

        if let Some(v) = pv {
            let version = v.value.trim().to_string();
            if !version.is_empty() {
                facts.insert("version".to_string(), FactValue::Version(version));
            }
        }

        // 渠道：channel 值为 "stable"（Evergreen）或缺省；Fixed 版本通道是 "fixed"
        let channel_str = channel
            .as_ref()
            .map(|c| c.value.trim().to_lowercase())
            .unwrap_or_else(|| "stable".to_string());
        let channel_name = if channel_str.is_empty() || channel_str == "stable" {
            "evergreen".to_string()
        } else {
            channel_str
        };
        facts.insert("channel".to_string(), FactValue::Str(channel_name.clone()));

        let (status, summary) = if installed {
            (
                DetectStatus::Ok,
                format!(
                    "WebView2 Runtime {} ({channel_name})",
                    pv.unwrap().value.trim()
                ),
            )
        } else {
            (
                DetectStatus::Absent,
                "WebView2 Runtime 未安装".to_string(),
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
pub fn parse_webview2(
    pv: Option<&str>,
    channel: Option<&str>,
) -> BTreeMap<String, FactValue> {
    let mut facts = BTreeMap::new();
    let installed = pv.is_some();
    facts.insert("installed".to_string(), FactValue::Bool(installed));

    if let Some(v) = pv {
        let version = v.trim().to_string();
        if !version.is_empty() {
            facts.insert("version".to_string(), FactValue::Version(version));
        }
    }

    let channel_str = channel.unwrap_or("").trim().to_lowercase();
    let channel_name = if channel_str.is_empty() || channel_str == "stable" {
        "evergreen".to_string()
    } else {
        channel_str
    };
    facts.insert("channel".to_string(), FactValue::Str(channel_name));

    facts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_installed_evergreen() {
        // 机器级安装，channel 缺省 → Evergreen
        let facts = parse_webview2(Some("120.0.2210.144"), None);
        assert_eq!(facts.get("installed").unwrap(), &FactValue::Bool(true));
        assert_eq!(
            facts.get("version").unwrap(),
            &FactValue::Version("120.0.2210.144".to_string())
        );
        assert_eq!(
            facts.get("channel").unwrap(),
            &FactValue::Str("evergreen".to_string())
        );
    }

    #[test]
    fn parse_fixed_channel() {
        // Fixed 版本通道
        let facts = parse_webview2(Some("116.0.1938.76"), Some("fixed"));
        assert_eq!(
            facts.get("channel").unwrap(),
            &FactValue::Str("fixed".to_string())
        );
    }

    #[test]
    fn parse_not_installed() {
        let facts = parse_webview2(None, None);
        assert_eq!(facts.get("installed").unwrap(), &FactValue::Bool(false));
        assert!(!facts.contains_key("version"));
    }

    #[test]
    fn parse_garbage_input() {
        let facts = parse_webview2(Some("  "), Some(" "));
        assert_eq!(facts.get("installed").unwrap(), &FactValue::Bool(true));
        assert!(!facts.contains_key("version"));
        assert_eq!(
            facts.get("channel").unwrap(),
            &FactValue::Str("evergreen".to_string())
        );
    }
}
