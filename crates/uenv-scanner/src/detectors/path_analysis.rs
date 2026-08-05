// path.analysis detector — PATH 条目总数、重复、缺失、被遮蔽的同名可执行文件。
// layer=Toolchain
// 规格 §5.4：PATH 原文进 volatile；facts 只放解析出的结构化结果。
// shadowed_exes 只收录命中 >1 处的关注名单（node/npm/npx/pnpm/yarn/bun/python/cargo/rustc/git/dotnet）。
// 只记录事实（哪个 exe 有多个命中），不判断「这是问题」——那是 T5 规则引擎的事。

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, Evidence, EvidenceKind, FactValue, Layer};

use crate::context::ScanContext;
use crate::detector::{Detector, DetectorMeta, DetectorResult};
use crate::util::find_all_in_path;

/// 遮蔽检测关注名单：Windows 开发最常见的冲突源
const WATCHLIST: &[&str] = &[
    "node", "npm", "npx", "pnpm", "yarn", "bun", "python", "cargo", "rustc", "git", "dotnet",
];

pub struct PathAnalysis;

impl Detector for PathAnalysis {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "path.analysis",
            layer: Layer::Toolchain,
            title: "PATH 分析",
            cost: Cost::Fast,
        }
    }

    fn detect(&self, ctx: &ScanContext) -> DetectorResult {
        let path_raw = std::env::var("PATH").unwrap_or_default();
        let path_redacted = if ctx.redact {
            ctx.redact(&path_raw)
        } else {
            path_raw.clone()
        };
        let mut evidence = Vec::new();
        evidence.push(Evidence {
            kind: EvidenceKind::Env,
            source: "PATH".to_string(),
            exit_code: None,
            excerpt: path_redacted.clone(),
        });

        // PATH 原文进 volatile（规格 §5.4：原文随终端而变，不进指纹）
        let mut volatile = BTreeMap::new();
        volatile.insert("path_raw".to_string(), FactValue::Str(path_redacted));

        let mut facts = BTreeMap::new();
        let entries: Vec<&str> = path_raw.split(';').filter(|e| !e.trim().is_empty()).collect();

        // 条目总数
        facts.insert("entry_count".to_string(), FactValue::Int(entries.len() as i64));

        // 重复条目（规范化比较：trim + 去尾部 \，大小写不敏感）
        let mut seen: std::collections::BTreeMap<String, i32> = BTreeMap::new();
        for e in &entries {
            let norm = normalize_entry(e);
            *seen.entry(norm).or_insert(0) += 1;
        }
        let duplicates: Vec<FactValue> = seen
            .iter()
            .filter(|(_, c)| **c > 1)
            .map(|(k, _)| FactValue::Str(k.clone()))
            .collect();
        if !duplicates.is_empty() {
            facts.insert("duplicates".to_string(), FactValue::Set(duplicates));
        }

        // 不存在的条目
        let missing: Vec<FactValue> = entries
            .iter()
            .map(|e| e.trim().trim_end_matches('\\'))
            .filter(|dir| !dir.is_empty() && !std::path::Path::new(dir).is_dir())
            .map(|dir| FactValue::Str(dir.to_string()))
            .collect();
        if !missing.is_empty() {
            facts.insert("missing".to_string(), FactValue::Set(missing));
        }

        // 遮蔽：关注名单里命中 >1 处的 exe
        let mut shadowed = BTreeMap::new();
        for exe in WATCHLIST {
            let hits = find_all_in_path(exe);
            if hits.len() > 1 {
                let paths: Vec<FactValue> = hits
                    .iter()
                    .map(|p| {
                        let s = p.to_string_lossy().to_string();
                        FactValue::Path(if ctx.redact { ctx.redact(&s) } else { s })
                    })
                    .collect();
                shadowed.insert(exe.to_string(), FactValue::Set(paths));
            }
        }
        if !shadowed.is_empty() {
            facts.insert("shadowed_exes".to_string(), FactValue::Map(shadowed));
        }

        let summary = format!(
            "PATH {} 条{}",
            entries.len(),
            if entries.is_empty() {
                "（空）".to_string()
            } else {
                String::new()
            }
        );

        DetectorResult {
            status: DetectStatus::Ok,
            summary,
            facts,
            volatile,
            evidence,
        }
    }
}

/// PATH 条目规范化：trim + 去尾部 \ + 小写（Windows 大小写不敏感）
fn normalize_entry(e: &str) -> String {
    e.trim().trim_end_matches('\\').to_lowercase()
}

/// 解析逻辑与 IO 分离 —— 独立可测。
/// 处理可纯字符串推导的部分：条目数、重复项。
/// missing（依赖文件系统）与 shadowed_exes（依赖 which 探测）在 detect() 里算。
pub fn parse_path_analysis(path_raw: &str) -> BTreeMap<String, FactValue> {
    let mut facts = BTreeMap::new();
    let entries: Vec<&str> = path_raw.split(';').filter(|e| !e.trim().is_empty()).collect();
    facts.insert("entry_count".to_string(), FactValue::Int(entries.len() as i64));

    // 重复
    let mut seen: BTreeMap<String, i32> = BTreeMap::new();
    for e in &entries {
        *seen.entry(normalize_entry(e)).or_insert(0) += 1;
    }
    let duplicates: Vec<FactValue> = seen
        .iter()
        .filter(|(_, c)| **c > 1)
        .map(|(k, _)| FactValue::Str(k.clone()))
        .collect();
    if !duplicates.is_empty() {
        facts.insert("duplicates".to_string(), FactValue::Set(duplicates));
    }

    facts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duplicates_and_count() {
        let facts = parse_path_analysis(r"C:\Windows;C:\Windows;D:\tools;");
        assert_eq!(facts.get("entry_count").unwrap(), &FactValue::Int(3));
        assert_eq!(
            facts.get("duplicates").unwrap(),
            &FactValue::Set(vec![FactValue::Str("c:\\windows".to_string())])
        );
    }

    #[test]
    fn parse_empty_path() {
        let facts = parse_path_analysis("");
        assert_eq!(facts.get("entry_count").unwrap(), &FactValue::Int(0));
        assert!(!facts.contains_key("duplicates"));
    }

    #[test]
    fn normalize_case_and_trailing() {
        assert_eq!(normalize_entry(r"C:\Program Files\"), "c:\\program files");
        assert_eq!(normalize_entry("  D:\\Tools  "), "d:\\tools");
    }
}
