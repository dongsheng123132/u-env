// security.defender detector — Windows Defender 实时保护状态、排除项是否覆盖项目目录。
// layer=Host
// 数据源：Get-MpComputerStatus / Get-MpPreference（PowerShell）。
// ⚠️ 非管理员读不到 ExclusionPath（返回 "N/A: Must be an administrator..."）→ Degraded，不是 Error。

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, EvidenceKind, FactValue, Layer};

use crate::context::{evidence_from_command, ScanContext};
use crate::detector::{Detector, DetectorMeta, DetectorResult};

pub struct SecurityDefender;

impl Detector for SecurityDefender {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "security.defender",
            layer: Layer::Host,
            title: "Windows Defender",
            cost: Cost::Slow,
        }
    }

    fn detect(&self, ctx: &ScanContext) -> DetectorResult {
        let mut evidence = Vec::new();

        // 实时保护状态
        let rt = ctx.run(
            "powershell",
            &[
                "-NoProfile",
                "-Command",
                "(Get-MpComputerStatus).RealTimeProtectionEnabled",
            ],
        );
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "powershell (Get-MpComputerStatus).RealTimeProtectionEnabled",
            &rt,
        ));

        // 排除路径（非管理员返回 N/A 字符串）
        let excl = ctx.run(
            "powershell",
            &[
                "-NoProfile",
                "-Command",
                "(Get-MpPreference).ExclusionPath | ConvertTo-Json -Compress",
            ],
        );
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "powershell (Get-MpPreference).ExclusionPath | ConvertTo-Json -Compress",
            &excl,
        ));

        let project_redacted = ctx
            .project_root
            .as_ref()
            .map(|p| ctx.redact(&p.to_string_lossy()));

        let (facts, degraded) = parse_defender(
            &rt.stdout,
            rt.ran,
            &excl.stdout,
            excl.ran,
            project_redacted.as_deref(),
        );

        let realtime = matches!(facts.get("realtime_enabled"), Some(FactValue::Bool(true)));
        let covers = matches!(
            facts.get("exclusion_covers_project"),
            Some(FactValue::Bool(true))
        );

        let (status, summary) = if degraded {
            (
                DetectStatus::Degraded,
                format!(
                    "实时保护 {}，排除项不可读（需管理员）",
                    if realtime { "开启" } else { "关闭" }
                ),
            )
        } else {
            (
                DetectStatus::Ok,
                format!(
                    "实时保护 {}，排除项覆盖项目{}",
                    if realtime { "开启" } else { "关闭" },
                    if covers { "是" } else { "否" }
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

/// 解析 Defender 输出 —— 与 IO 分离，独立可测。
///
/// 返回 (facts, degraded)。degraded=true 表示排除项因权限不可读。
pub fn parse_defender(
    realtime_stdout: &str,
    realtime_ran: bool,
    exclusions_stdout: &str,
    exclusions_ran: bool,
    project_redacted: Option<&str>,
) -> (BTreeMap<String, FactValue>, bool) {
    let mut facts = BTreeMap::new();
    let mut degraded = false;

    // 实时保护：True/False
    if realtime_ran {
        let t = realtime_stdout.trim();
        if t.eq_ignore_ascii_case("true") {
            facts.insert("realtime_enabled".to_string(), FactValue::Bool(true));
        } else if t.eq_ignore_ascii_case("false") {
            facts.insert("realtime_enabled".to_string(), FactValue::Bool(false));
        }
    }

    // 排除路径：JSON 数组 / null / "N/A: Must be an administrator..."
    let mut paths: Vec<String> = Vec::new();
    if exclusions_ran {
        let t = exclusions_stdout.trim();
        let is_admin_na = t.contains("N/A") || t.contains("administrator");
        if !is_admin_na {
            match serde_json::from_str::<serde_json::Value>(t) {
                Ok(serde_json::Value::Array(arr)) => {
                    for item in arr {
                        if let Some(s) = item.as_str() {
                            if !s.is_empty() {
                                paths.push(s.to_string());
                            }
                        }
                    }
                }
                Ok(serde_json::Value::String(s)) => {
                    // 单个字符串（无排除项时可能输出 null，单个值时是字符串）
                    if !s.is_empty() && !s.eq_ignore_ascii_case("null") {
                        paths.push(s);
                    }
                }
                _ => {}
            }
        } else {
            degraded = true;
        }
    } else {
        degraded = true;
    }

    // 排除路径集合（进 facts，Set）
    if !paths.is_empty() {
        let set: Vec<FactValue> = paths
            .iter()
            .map(|p| FactValue::Path(p.clone()))
            .collect();
        facts.insert("exclusion_paths".to_string(), FactValue::Set(set));
    }

    // 排除项是否覆盖项目目录：任一排除路径是项目路径的前缀（路径边界）
    let covers = if let Some(proj) = project_redacted {
        paths.iter().any(|excl| path_prefix_covers(excl, proj))
    } else {
        false
    };
    facts.insert(
        "exclusion_covers_project".to_string(),
        FactValue::Bool(covers),
    );

    (facts, degraded)
}

/// 判断排除路径 excl 是否覆盖路径 proj（前缀匹配，要求路径边界）。
/// 统一分隔符后比较；excl 尾部可带可不带分隔符。
fn path_prefix_covers(excl: &str, proj: &str) -> bool {
    let norm = |s: &str| s.replace('\\', "/").trim_end_matches('/').to_lowercase();
    let e = norm(excl);
    let p = norm(proj);
    if p == e {
        return true;
    }
    p.starts_with(&e) && p[e.len()..].starts_with('/')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_realtime_on_no_exclusions() {
        // 管理员，实时保护开，无排除项
        let (facts, degraded) = parse_defender("True", true, "null", true, Some(r"C:\proj"));
        assert_eq!(
            facts.get("realtime_enabled").unwrap(),
            &FactValue::Bool(true)
        );
        assert!(!facts.contains_key("exclusion_paths"));
        assert_eq!(
            facts.get("exclusion_covers_project").unwrap(),
            &FactValue::Bool(false)
        );
        assert!(!degraded);
    }

    #[test]
    fn parse_exclusion_covers_project() {
        let (facts, degraded) = parse_defender(
            "False",
            true,
            r#"["C:\\Users\\me\\Tools","D:\\proj"]"#,
            true,
            Some(r"D:\proj\my-app"),
        );
        assert_eq!(
            facts.get("realtime_enabled").unwrap(),
            &FactValue::Bool(false)
        );
        assert_eq!(
            facts.get("exclusion_paths").unwrap(),
            &FactValue::Set(vec![
                FactValue::Path(r"C:\Users\me\Tools".to_string()),
                FactValue::Path(r"D:\proj".to_string()),
            ])
        );
        assert_eq!(
            facts.get("exclusion_covers_project").unwrap(),
            &FactValue::Bool(true)
        );
        assert!(!degraded);
    }

    #[test]
    fn parse_exclusion_not_cover_sibling() {
        // D:\proj-other 不是 D:\proj 的子路径（路径边界）
        let (facts, _) = parse_defender(
            "True",
            true,
            r#"["D:\\proj"]"#,
            true,
            Some(r"D:\proj-other"),
        );
        assert_eq!(
            facts.get("exclusion_covers_project").unwrap(),
            &FactValue::Bool(false)
        );
    }

    #[test]
    fn parse_non_admin_degraded() {
        // 非管理员：ExclusionPath 返回 N/A 字符串
        let (facts, degraded) = parse_defender(
            "True",
            true,
            r#""N/A: Must be an administrator to view exclusions""#,
            true,
            Some(r"C:\proj"),
        );
        assert!(degraded);
        assert!(!facts.contains_key("exclusion_paths"));
        assert_eq!(
            facts.get("exclusion_covers_project").unwrap(),
            &FactValue::Bool(false)
        );
    }

    #[test]
    fn parse_garbage_input() {
        // 畸形输入：不 panic
        let (facts, degraded) = parse_defender("not-a-bool", false, "{{{", false, None);
        assert!(facts.is_empty() || !facts.contains_key("realtime_enabled"));
        assert!(degraded);
    }
}
