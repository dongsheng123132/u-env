// wsl.status detector — WSL 是否安装、默认版本、发行版列表、内核版本。
// layer=Host
// 数据源：wsl --status / wsl --list --verbose / wsl --version
// ⚠️ wsl.exe 输出是无 BOM 的 UTF-16LE（中英混合），依赖 util.rs 的解码器。

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, EvidenceKind, FactValue, Layer};

use crate::context::{evidence_from_command, ScanContext};
use crate::detector::{Detector, DetectorMeta, DetectorResult};

pub struct WslStatus;

impl Detector for WslStatus {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "wsl.status",
            layer: Layer::Host,
            title: "WSL 状态",
            cost: Cost::Slow,
        }
    }

    fn detect(&self, ctx: &ScanContext) -> DetectorResult {
        let mut evidence = Vec::new();

        // wsl --status：安装状态 + 默认版本（中文系统："默认版本: 2" / 英文："Default Version: 2"）
        let status = ctx.run("wsl", &["--status"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "wsl --status",
            &status,
        ));

        if !status.ran {
            return DetectorResult {
                status: DetectStatus::Ok,
                summary: "WSL 未安装（wsl.exe 不在 PATH）".to_string(),
                facts: BTreeMap::from([(
                    "installed".to_string(),
                    FactValue::Bool(false),
                )]),
                volatile: BTreeMap::new(),
                evidence,
            };
        }

        // wsl --list --verbose：发行版列表（等宽表格，* 标记默认发行版）
        let list = ctx.run("wsl", &["--list", "--verbose"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "wsl --list --verbose",
            &list,
        ));

        // wsl --version：内核版本（中文系统："内核版本： 5.15.167.4-1" / 英文："Kernel Version: ..."）
        // 注意：wsl --version 较新版本才支持，失败不影响主状态。
        let version = ctx.run("wsl", &["--version"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "wsl --version",
            &version,
        ));

        let facts = parse_wsl(
            &status.stdout,
            &list.stdout,
            &version.stdout,
            version.ran,
        );

        let installed = matches!(
            facts.get("installed"),
            Some(FactValue::Bool(true))
        );
        let default_version = facts
            .get("default_version")
            .map(|v| match v {
                FactValue::Str(s) => s.clone(),
                _ => String::new(),
            })
            .unwrap_or_default();
        let distro_count = facts
            .get("distros")
            .map(|v| match v {
                FactValue::Set(s) => s.len(),
                _ => 0,
            })
            .unwrap_or(0);

        let (status, summary) = if !installed {
            (
                DetectStatus::Ok,
                "WSL 已安装但未检测到发行版".to_string(),
            )
        } else {
            (
                DetectStatus::Ok,
                format!(
                    "WSL 已安装, 默认版本 {}, {} 个发行版",
                    if default_version.is_empty() {
                        "未知".to_string()
                    } else {
                        default_version
                    },
                    distro_count
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

/// 解析 wsl 三连输出的核心逻辑 —— 与 IO 分离，独立可测。
///
/// 兼容中文/英文系统：中文用全角或半角冒号（"默认版本: 2" / "默认版本： 2"），
/// 英文用 "Default Version: 2"。`-l -v` 是等宽表格，首列 NAME（* 标记默认）。
pub fn parse_wsl(
    status_out: &str,
    list_out: &str,
    version_out: &str,
    version_ran: bool,
) -> BTreeMap<String, FactValue> {
    let mut facts = BTreeMap::new();

    // 1. 安装状态：wsl --status 跑通且有输出 → 已安装
    let installed = !status_out.trim().is_empty();
    facts.insert("installed".to_string(), FactValue::Bool(installed));

    // 2. 默认版本：默认版本: 2 / 默认版本： 2 / Default Version: 2
    if let Some(v) = extract_field(status_out, &["默认版本", "Default Version"]) {
        facts.insert("default_version".to_string(), FactValue::Str(v));
    }

    // 3. 发行版列表：跳过表头行（NAME/STATE/VERSION），每行首列是发行版名。
    //    `-l -v` 是等宽表格，列间用多个空格分隔；纯文本提示行
    //    （如 "Windows Subsystem for Linux 没有已安装的分发版。"）没有
    //    表格列分隔（>=2 空格），据此排除。
    let mut distros = Vec::new();
    for line in list_out.lines() {
        let line = line.trim();
        if line.is_empty()
            || !line.contains("  ")
            || line.to_lowercase().starts_with("name ")
        {
            continue;
        }
        // 去掉 * 标记（默认发行版），取第一个 token
        let name = line
            .trim_start_matches('*')
            .trim()
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();
        if !name.is_empty() {
            distros.push(FactValue::Str(name));
        }
    }
    if !distros.is_empty() {
        facts.insert("distros".to_string(), FactValue::Set(distros));
    }

    // 4. 内核版本：wsl --version 的"内核版本"行（该命令在新版 WSL 才支持）
    if version_ran {
        if let Some(v) = extract_field(version_out, &["内核版本", "Kernel Version"]) {
            facts.insert("kernel_version".to_string(), FactValue::Str(v));
        }
    }

    facts
}

/// 从 key: value 或 key： value 形式的行中提取 value（兼容全角/半角冒号 + 任意空白）
/// key 匹配大小写不敏感（英文 WSL 输出混用 "Kernel Version"/"Kernel version"）。
fn extract_field(text: &str, keys: &[&str]) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        let lower_line = line.to_lowercase();
        for key in keys {
            let lower_key = key.to_lowercase();
            // 兼容 "key: value"（半角）和 "key： value"（全角）
            for sep in [":", "："] {
                if let Some(sep_start) = line.find(sep) {
                    // key 必须在冒号前且以行首开始（允许前面有空白，已 trim）
                    if lower_line.starts_with(&lower_key) {
                        // 冒号必须紧跟 key 后面（允许 key 与冒号间有空白）
                        let key_end = lower_key.len();
                        let between = &line[key_end..sep_start];
                        if between.trim().is_empty() {
                            let v = line[sep_start + sep.len()..].trim();
                            if !v.is_empty() {
                                return Some(v.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_zh_wsl2_with_ubuntu() {
        // 中文系统实测输出
        let status = "默认分发: Ubuntu\r\n默认版本: 2\r\n";
        let list = "  NAME      STATE           VERSION\r\n* Ubuntu    Stopped         2\r\n";
        let version = "WSL 版本： 2.4.13.0\r\n内核版本： 5.15.167.4-1\r\n";
        let facts = parse_wsl(status, list, version, true);
        assert_eq!(
            facts.get("installed").unwrap(),
            &FactValue::Bool(true)
        );
        assert_eq!(
            facts.get("default_version").unwrap(),
            &FactValue::Str("2".to_string())
        );
        assert_eq!(
            facts.get("distros").unwrap(),
            &FactValue::Set(vec![FactValue::Str("Ubuntu".to_string())])
        );
        assert_eq!(
            facts.get("kernel_version").unwrap(),
            &FactValue::Str("5.15.167.4-1".to_string())
        );
    }

    #[test]
    fn parse_en_system() {
        // 英文系统输出
        let status = "Default Distribution: Ubuntu\nDefault Version: 2\n";
        let list = "  NAME      STATE           VERSION\n* Ubuntu    Stopped         2\n";
        let version = "WSL version: 2.4.13.0\nKernel version: 5.15.167.4-1\n";
        let facts = parse_wsl(status, list, version, true);
        assert_eq!(
            facts.get("default_version").unwrap(),
            &FactValue::Str("2".to_string())
        );
        assert_eq!(
            facts.get("kernel_version").unwrap(),
            &FactValue::Str("5.15.167.4-1".to_string())
        );
    }

    #[test]
    fn parse_not_installed() {
        // wsl.exe 不存在 → 空输出
        let facts = parse_wsl("", "", "", false);
        assert_eq!(
            facts.get("installed").unwrap(),
            &FactValue::Bool(false)
        );
        assert!(!facts.contains_key("default_version"));
        assert!(!facts.contains_key("distros"));
    }

    #[test]
    fn parse_no_distros() {
        // WSL 装了但没有任何发行版（旧版输出 "没有已安装的分发版"）
        let status = "默认版本: 2\r\n";
        let list = "Windows Subsystem for Linux 没有已安装的分发版。\r\n";
        let facts = parse_wsl(status, list, "", false);
        assert!(!facts.contains_key("distros"));
        assert_eq!(
            facts.get("default_version").unwrap(),
            &FactValue::Str("2".to_string())
        );
    }

    #[test]
    fn parse_multiple_distros() {
        // 多发行版 + 全角冒号 + 名字带连字符
        let status = "默认版本： 2\r\n";
        let list = "  NAME          STATE           VERSION\r\n* Ubuntu-22.04  Running         2\r\n  Debian        Stopped         2\r\n";
        let facts = parse_wsl(status, list, "", false);
        assert_eq!(
            facts.get("distros").unwrap(),
            &FactValue::Set(vec![
                FactValue::Str("Ubuntu-22.04".to_string()),
                FactValue::Str("Debian".to_string()),
            ])
        );
    }
}
