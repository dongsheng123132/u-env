// toolchain.python detector — python/py launcher 版本与路径、Microsoft Store 别名坑。
// layer=Toolchain
// Store 别名坑：%LOCALAPPDATA%\Microsoft\WindowsApps\python.exe 是 App Installer 的
// 重定向 stub（0 字节或指向 AppInstallerPythonRedirector.exe），点开会弹商店。
// 命中它要标 store_alias_shadow=true —— 只记录事实，判断留给 T5。

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, EvidenceKind, FactValue, Layer};

use crate::context::{evidence_from_command, ScanContext};
use crate::detector::{Detector, DetectorMeta, DetectorResult};

pub struct ToolchainPython;

impl Detector for ToolchainPython {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "toolchain.python",
            layer: Layer::Toolchain,
            title: "Python",
            cost: Cost::Slow,
        }
    }

    fn detect(&self, ctx: &ScanContext) -> DetectorResult {
        let mut evidence = Vec::new();

        // 所有 PATH 中的 python
        let hits = ctx.which_all("python");

        // python --version（用 PATH 中第一个）
        let py_ver = ctx.run("python", &["--version"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "python --version",
            &py_ver,
        ));

        // py launcher（Windows 官方 launcher，可选）
        let py_launcher = ctx.run("py", &["--version"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "py --version",
            &py_launcher,
        ));

        // Store 别名坑：WindowsApps 目录下的 python.exe 是重定向 stub
        let store_alias = detect_store_alias(&hits);

        let mut final_facts = parse_python(
            &py_ver.stdout,
            py_ver.ran,
            &py_launcher.stdout,
            py_launcher.ran,
            store_alias,
        );

        // python_paths 是 which_all 的 IO 结果（已脱敏），合并进 facts
        if !hits.is_empty() {
            let paths: Vec<FactValue> = hits
                .iter()
                .map(|p| FactValue::Path(p.to_string_lossy().to_string()))
                .collect();
            final_facts.insert("python_paths".to_string(), FactValue::Set(paths));
        }

        let py_installed = final_facts.contains_key("python_version");
        let (status, summary) = if py_installed {
            let v = match final_facts.get("python_version").unwrap() {
                FactValue::Version(s) => s.clone(),
                _ => String::new(),
            };
            (
                DetectStatus::Ok,
                if matches!(final_facts.get("store_alias_shadow"), Some(FactValue::Bool(true))) {
                    format!("Python {v}（注意：PATH 含 Store 别名）")
                } else {
                    format!("Python {v}")
                },
            )
        } else if !hits.is_empty() {
            (
                DetectStatus::Degraded,
                "python 在 PATH 但 --version 失败（可能是 Store 别名 stub）".to_string(),
            )
        } else {
            (
                DetectStatus::Absent,
                "Python 未安装（python/py 均不在 PATH）".to_string(),
            )
        };

        DetectorResult {
            status,
            summary,
            facts: final_facts,
            volatile: BTreeMap::new(),
            evidence,
        }
    }
}

/// 检测 Store 别名 stub：命中路径在 %LOCALAPPDATA%\Microsoft\WindowsApps 下
fn detect_store_alias(hits: &[std::path::PathBuf]) -> bool {
    let apps_dir = std::env::var("LOCALAPPDATA")
        .map(|d| format!("{}\\Microsoft\\WindowsApps", d.trim_end_matches('\\')).to_lowercase())
        .unwrap_or_default();
    hits.iter().any(|p| {
        let s = p.to_string_lossy().to_lowercase();
        s.starts_with(&apps_dir) && s.contains("windowsapps")
    })
}

/// 解析逻辑与 IO 分离 —— 独立可测
pub fn parse_python(
    py_ver_stdout: &str,
    py_ver_ran: bool,
    launcher_stdout: &str,
    launcher_ran: bool,
    store_alias: bool,
) -> BTreeMap<String, FactValue> {
    let mut facts = BTreeMap::new();

    if py_ver_ran {
        // "Python 3.12.7" → "3.12.7"
        let v = py_ver_stdout.trim();
        if let Some(ver) = v.strip_prefix("Python ") {
            if !ver.is_empty() {
                facts.insert("python_version".to_string(), FactValue::Version(ver.to_string()));
            }
        }
    }
    if launcher_ran {
        let v = launcher_stdout.trim();
        if let Some(ver) = v.strip_prefix("Python ") {
            if !ver.is_empty() {
                facts.insert("py_launcher_version".to_string(), FactValue::Version(ver.to_string()));
            }
        }
    }
    facts.insert("store_alias_shadow".to_string(), FactValue::Bool(store_alias));

    facts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_with_launcher() {
        let facts = parse_python("Python 3.12.7\r\n", true, "Python 3.11.9\r\n", true, false);
        assert_eq!(
            facts.get("python_version").unwrap(),
            &FactValue::Version("3.12.7".to_string())
        );
        assert_eq!(
            facts.get("py_launcher_version").unwrap(),
            &FactValue::Version("3.11.9".to_string())
        );
        assert_eq!(
            facts.get("store_alias_shadow").unwrap(),
            &FactValue::Bool(false)
        );
    }

    #[test]
    fn parse_store_alias_shadow() {
        // py 输出 "Python 3.11.9"，python 指向 Store 别名
        let facts = parse_python("", false, "Python 3.11.9\r\n", true, true);
        assert_eq!(
            facts.get("store_alias_shadow").unwrap(),
            &FactValue::Bool(true)
        );
        assert!(!facts.contains_key("python_version"));
    }

    #[test]
    fn parse_garbage_input() {
        // 畸形输入：不 panic
        let facts = parse_python("not python output", true, "", false, false);
        assert!(!facts.contains_key("python_version"));
        assert_eq!(
            facts.get("store_alias_shadow").unwrap(),
            &FactValue::Bool(false)
        );
    }
}
