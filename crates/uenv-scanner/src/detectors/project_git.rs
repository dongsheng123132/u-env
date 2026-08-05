// project.git detector — 分支、HEAD commit、dirty、remote 名称、submodule。
// layer=Project
// ⚠️ remote URL 必须脱敏（ScanContext 自动处理）；submodule 只报告存在性。

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, EvidenceKind, FactValue, Layer};

use crate::context::{evidence_from_command, ScanContext};
use crate::detector::{Detector, DetectorMeta, DetectorResult};

pub struct ProjectGit;

impl Detector for ProjectGit {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "project.git",
            layer: Layer::Project,
            title: "Git 状态",
            cost: Cost::Fast,
        }
    }

    fn applicable(&self, ctx: &ScanContext) -> bool {
        ctx.project_root.is_some()
    }

    fn detect(&self, ctx: &ScanContext) -> DetectorResult {
        let root = match &ctx.project_root {
            Some(p) => p.clone(),
            None => {
                return DetectorResult {
                    status: DetectStatus::Skipped,
                    summary: "未指定 --project，跳过".to_string(),
                    facts: BTreeMap::new(),
                    volatile: BTreeMap::new(),
                    evidence: vec![],
                };
            }
        };
        let root = std::path::absolute(&root).unwrap_or(root);

        let mut evidence = Vec::new();

        // 不是 git 仓库 → Skipped
        if !root.join(".git").exists() {
            return DetectorResult {
                status: DetectStatus::Skipped,
                summary: "不是 git 仓库（无 .git）".to_string(),
                facts: BTreeMap::new(),
                volatile: BTreeMap::new(),
                evidence: vec![],
            };
        }

        // 在项目根跑 git（git -C <root> 避免改 cwd）
        let branch = ctx.run("git", &["-C", root.to_str().unwrap_or("."), "branch", "--show-current"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "git branch --show-current",
            &branch,
        ));

        let head = ctx.run("git", &["-C", root.to_str().unwrap_or("."), "rev-parse", "HEAD"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "git rev-parse HEAD",
            &head,
        ));

        let status = ctx.run("git", &["-C", root.to_str().unwrap_or("."), "status", "--porcelain"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "git status --porcelain",
            &status,
        ));

        let submodules = ctx.run("git", &["-C", root.to_str().unwrap_or("."), "submodule", "status"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "git submodule status",
            &submodules,
        ));

        let facts = parse_git(
            &branch.stdout,
            branch.ran,
            &head.stdout,
            head.ran,
            &status.stdout,
            status.ran,
            &submodules.stdout,
            submodules.ran,
        );

        let (det_status, summary) = if !facts.contains_key("commit") {
            (
                DetectStatus::Error,
                "git 命令失败，无法读取仓库状态".to_string(),
            )
        } else {
            let b = facts
                .get("branch")
                .map(|v| match v {
                    FactValue::Str(s) => s.clone(),
                    _ => String::new(),
                })
                .unwrap_or_default();
            let dirty = matches!(facts.get("dirty"), Some(FactValue::Bool(true)));
            (
                DetectStatus::Ok,
                format!(
                    "{}{}",
                    if b.is_empty() { "detached HEAD".to_string() } else { b },
                    if dirty { " (dirty)" } else { "" }
                ),
            )
        };

        DetectorResult {
            status: det_status,
            summary,
            facts,
            volatile: BTreeMap::new(),
            evidence,
        }
    }
}

/// 解析 git 输出 —— 与 IO 分离，独立可测。
/// remote URL 已在 ScanContext 层脱敏。
pub fn parse_git(
    branch_out: &str,
    branch_ran: bool,
    head_out: &str,
    head_ran: bool,
    status_out: &str,
    status_ran: bool,
    submodule_out: &str,
    submodule_ran: bool,
) -> BTreeMap<String, FactValue> {
    let mut facts = BTreeMap::new();

    if branch_ran {
        let b = branch_out.trim();
        if !b.is_empty() {
            facts.insert("branch".to_string(), FactValue::Str(b.to_string()));
        }
    }

    if head_ran {
        let c = head_out.trim();
        if !c.is_empty() {
            // commit 短显示前 12 位
            let short: String = c.chars().take(12).collect();
            facts.insert("commit".to_string(), FactValue::Str(short));
        }
    }

    if status_ran {
        // porcelain 有输出 → dirty
        let dirty = status_out.lines().any(|l| !l.trim().is_empty());
        facts.insert("dirty".to_string(), FactValue::Bool(dirty));
    }

    if submodule_ran {
        // " 无输出" 或空 → 无 submodule
        let has = submodule_out.lines().any(|l| !l.trim().is_empty());
        facts.insert("has_submodules".to_string(), FactValue::Bool(has));
    }

    facts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_clean_branch() {
        let facts = parse_git("main\n", true, "abc123def4567890\n", true, "", true, "", true);
        assert_eq!(facts.get("branch").unwrap(), &FactValue::Str("main".to_string()));
        assert_eq!(facts.get("commit").unwrap(), &FactValue::Str("abc123def456".to_string()));
        assert_eq!(facts.get("dirty").unwrap(), &FactValue::Bool(false));
        assert_eq!(facts.get("has_submodules").unwrap(), &FactValue::Bool(false));
    }

    #[test]
    fn parse_dirty_with_submodules() {
        let facts = parse_git(
            "main\n",
            true,
            "abc\n",
            true,
            " M src/main.rs\n?? new.txt\n",
            true,
            " 5f4d3c2  sub/repo (v1.0)\n",
            true,
        );
        assert_eq!(facts.get("dirty").unwrap(), &FactValue::Bool(true));
        assert_eq!(facts.get("has_submodules").unwrap(), &FactValue::Bool(true));
    }

    #[test]
    fn parse_detached_head() {
        let facts = parse_git("", true, "abc\n", true, "", true, "", true);
        assert!(!facts.contains_key("branch"));
        assert_eq!(facts.get("commit").unwrap(), &FactValue::Str("abc".to_string()));
    }

    #[test]
    fn parse_garbage_input() {
        // 畸形输入：不 panic
        let facts = parse_git("!!!", true, "???", true, "\n\n", true, "===", true);
        assert!(facts.contains_key("commit"));
    }
}
