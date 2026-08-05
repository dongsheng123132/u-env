// toolchain.git detector — Git 版本与关键配置（autocrlf/longpaths/symlinks/safe.directory）。
// layer=Toolchain
// 数据源：git --version、git config --list --show-origin
// ⚠️ git config 输出含 user.email 等隐私 → ScanContext 已脱敏（redact_emails）。

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, EvidenceKind, FactValue, Layer};

use crate::context::{ScanContext, evidence_from_command};
use crate::detector::{Detector, DetectorMeta, DetectorResult};

pub struct ToolchainGit;

impl Detector for ToolchainGit {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "toolchain.git",
            layer: Layer::Toolchain,
            title: "Git",
            cost: Cost::Fast,
        }
    }

    fn detect(&self, ctx: &ScanContext) -> DetectorResult {
        let mut evidence = Vec::new();

        let ver = ctx.run("git", &["--version"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "git --version",
            &ver,
        ));

        // 未安装 → Absent（不是 Error）
        if !ver.ran {
            return DetectorResult {
                status: DetectStatus::Absent,
                summary: "Git 未安装（git 不在 PATH）".to_string(),
                facts: BTreeMap::new(),
                volatile: BTreeMap::new(),
                evidence,
            };
        }

        let config = ctx.run("git", &["config", "--list", "--show-origin"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "git config --list --show-origin",
            &config,
        ));

        let version = parse_git_version(&ver.stdout);
        let facts = if version.is_empty() {
            BTreeMap::new()
        } else {
            parse_git_config(&config.stdout, &version)
        };

        let (status, summary) = if version.is_empty() {
            (
                DetectStatus::Error,
                "git --version 输出无法解析".to_string(),
            )
        } else {
            (DetectStatus::Ok, format!("Git {version}"))
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

/// git --version → "2.49.0.windows.1"
fn parse_git_version(out: &str) -> String {
    let line = out.lines().next().unwrap_or("");
    let lower = line.to_lowercase();
    match lower.find("git version ") {
        Some(idx) => line[idx + "git version ".len()..].trim().to_string(),
        None => String::new(),
    }
}

/// 解析 config --list --show-origin 输出。
/// 行格式：`file:C:/path\tkey=value`（git 用 tab 分隔 origin 与 key=value）。
pub fn parse_git_config(out: &str, version: &str) -> BTreeMap<String, FactValue> {
    let mut facts = BTreeMap::new();
    facts.insert(
        "version".to_string(),
        FactValue::Version(version.to_string()),
    );

    let mut autocrlf: Option<String> = None;
    let mut longpaths: Option<String> = None;
    let mut symlinks: Option<String> = None;
    let mut safe_dirs: Vec<FactValue> = Vec::new();

    for line in out.lines() {
        let (_, kv) = match line.split_once('\t') {
            Some((origin, kv)) => (origin, kv),
            None => ("", line), // 无 origin（罕见），直接当 key=value
        };
        let (key, value) = match kv.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => (kv.trim(), ""),
        };
        match key {
            "core.autocrlf" => autocrlf = Some(value.to_string()),
            "core.longpaths" => longpaths = Some(value.to_string()),
            "core.symlinks" => symlinks = Some(value.to_string()),
            "safe.directory" => {
                if !value.is_empty() {
                    safe_dirs.push(FactValue::Path(value.to_string()));
                }
            }
            _ => {}
        }
    }

    if let Some(v) = autocrlf {
        facts.insert("autocrlf".to_string(), FactValue::Str(v));
    }
    if let Some(v) = longpaths {
        facts.insert("longpaths".to_string(), FactValue::Str(v));
    }
    if let Some(v) = symlinks {
        facts.insert("symlinks".to_string(), FactValue::Str(v));
    }
    if !safe_dirs.is_empty() {
        // Set 语义：去重（规范化层也会做，但这里保持输出干净）
        safe_dirs.sort_by(|a, b| {
            let sa = match a {
                FactValue::Path(p) => p.as_str(),
                _ => "",
            };
            let sb = match b {
                FactValue::Path(p) => p.as_str(),
                _ => "",
            };
            sa.cmp(sb)
        });
        safe_dirs.dedup();
        facts.insert("safe_directories".to_string(), FactValue::Set(safe_dirs));
    }

    facts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_windows() {
        assert_eq!(
            parse_git_version("git version 2.49.0.windows.1\n"),
            "2.49.0.windows.1"
        );
        assert_eq!(parse_git_version(""), "");
    }

    #[test]
    fn parse_config_full() {
        let out = "file:C:/Program Files/Git/etc/gitconfig\tcore.autocrlf=true\n\
                   file:C:/Program Files/Git/etc/gitconfig\tcore.symlinks=false\n\
                   file:C:/Users/<user>/.gitconfig\tuser.name=someone\n\
                   file:C:/Users/<user>/.gitconfig\tuser.email=<redacted>\n\
                   file:C:/Users/<user>/.gitconfig\tsafe.directory=C:/Users/<user>/proj\n";
        let facts = parse_git_config(out, "2.49.0.windows.1");
        assert_eq!(
            facts.get("version").unwrap(),
            &FactValue::Version("2.49.0.windows.1".to_string())
        );
        assert_eq!(
            facts.get("autocrlf").unwrap(),
            &FactValue::Str("true".to_string())
        );
        assert_eq!(
            facts.get("symlinks").unwrap(),
            &FactValue::Str("false".to_string())
        );
        assert_eq!(
            facts.get("safe_directories").unwrap(),
            &FactValue::Set(vec![FactValue::Path("C:/Users/<user>/proj".to_string())])
        );
        // longpaths 未配置 → 无键
        assert!(!facts.contains_key("longpaths"));
    }

    #[test]
    fn parse_config_garbage() {
        // 畸形输入：不 panic，只保留 version
        let facts = parse_git_config("not a config line\n===garbage===\n", "2.49.0");
        assert_eq!(
            facts.get("version").unwrap(),
            &FactValue::Version("2.49.0".to_string())
        );
        assert!(!facts.contains_key("autocrlf"));
    }
}
