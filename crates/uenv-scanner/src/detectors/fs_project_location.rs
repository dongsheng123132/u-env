// fs.project-location detector — 项目根位置特征：盘符、文件系统、OneDrive/网络盘/WSL 路径、
// 中文或空格路径。
// layer=Host
// 数据源：fsutil fsinfo volumeinfo <drive>（失败时退回 PowerShell Get-Volume）+ 路径前缀判断。

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, Evidence, EvidenceKind, FactValue, Layer};

use crate::context::{ScanContext, evidence_from_command};
use crate::detector::{Detector, DetectorMeta, DetectorResult};

pub struct FsProjectLocation;

impl Detector for FsProjectLocation {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "fs.project-location",
            layer: Layer::Host,
            title: "项目位置特征",
            cost: Cost::Slow,
        }
    }

    fn applicable(&self, ctx: &ScanContext) -> bool {
        ctx.project_root.is_some()
    }

    fn detect(&self, ctx: &ScanContext) -> DetectorResult {
        let project_root = match &ctx.project_root {
            Some(p) => p.clone(),
            None => {
                return DetectorResult {
                    status: DetectStatus::Skipped,
                    summary: "无项目根，跳过".to_string(),
                    facts: BTreeMap::new(),
                    volatile: BTreeMap::new(),
                    evidence: vec![],
                };
            }
        };

        let mut evidence = Vec::new();
        // 相对路径（如 --project .）先规范成绝对路径，否则解析不出盘符
        let project_root = std::path::absolute(&project_root).unwrap_or(project_root);
        let root_str = project_root.to_string_lossy().to_string();

        // 盘符：C:\... → C:
        let drive = root_str
            .chars()
            .next()
            .filter(|c| c.is_ascii_alphabetic())
            .map(|c| format!("{c}:"))
            .unwrap_or_default();

        // 文件系统类型：fsutil 优先（非管理员可能被拒），失败退 PowerShell
        let fs = detect_filesystem(ctx, &drive, &mut evidence);

        // 路径特征（脱敏后进 evidence）
        let redacted_root = ctx.redact(&root_str);
        evidence.push(Evidence {
            kind: EvidenceKind::File,
            source: redacted_root.clone(),
            exit_code: None,
            excerpt: redacted_root.clone(),
        });

        let facts = parse_project_location(&root_str, &fs);

        let summary = format!(
            "{} on {} ({}){}{}{}",
            drive,
            if fs.is_empty() { "unknown" } else { &fs },
            root_str,
            if matches!(facts.get("on_onedrive"), Some(FactValue::Bool(true))) {
                ", OneDrive"
            } else {
                ""
            },
            if matches!(facts.get("on_network"), Some(FactValue::Bool(true))) {
                ", 网络盘"
            } else {
                ""
            },
            if matches!(facts.get("on_wsl"), Some(FactValue::Bool(true))) {
                ", WSL 路径"
            } else {
                ""
            },
        );

        DetectorResult {
            status: DetectStatus::Ok,
            summary,
            facts,
            volatile: BTreeMap::new(),
            evidence,
        }
    }
}

/// 文件系统类型：fsutil fsinfo volumeinfo <drive> 优先，失败（非管理员/无权限）
/// 退回 PowerShell Get-Volume。
fn detect_filesystem(ctx: &ScanContext, drive: &str, evidence: &mut Vec<Evidence>) -> String {
    if !drive.is_empty() {
        let fsutil = ctx.run("fsutil", &["fsinfo", "volumeinfo", drive]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "fsutil fsinfo volumeinfo <drive>",
            &fsutil,
        ));
        if fsutil.ran && fsutil.exit_code == Some(0) {
            if let Some(fs) = parse_fsutil(&fsutil.stdout) {
                return fs;
            }
        }
    }

    // 兜底：Get-Volume（不需要管理员）
    let ps = if !drive.is_empty() {
        let letter = drive.trim_end_matches(':');
        ctx.run(
            "powershell",
            &[
                "-NoProfile",
                "-Command",
                &format!("(Get-Volume -DriveLetter {letter}).FileSystem"),
            ],
        )
    } else {
        ctx.run(
            "powershell",
            &[
                "-NoProfile",
                "-Command",
                "Get-Volume | ConvertTo-Json -Compress",
            ],
        )
    };
    evidence.push(evidence_from_command(
        EvidenceKind::Command,
        "powershell Get-Volume",
        &ps,
    ));
    if ps.ran && ps.exit_code == Some(0) {
        if let Some(fs) = parse_fsutil(&ps.stdout) {
            return fs;
        }
        if let Some(fs) = parse_volume_json(&ps.stdout) {
            return fs;
        }
        // 裸值输出：单行非空、无冒号（如 "(Get-Volume -DriveLetter D).FileSystem" → "NTFS"）
        let bare = ps.stdout.trim();
        if !bare.is_empty() && !bare.contains(':') && !bare.contains('\n') {
            return bare.to_string();
        }
    }
    String::new()
}

/// fsutil 输出（本地化）：英文 "File System Name : NTFS"，中文 "文件系统名称 : NTFS"
fn parse_fsutil(out: &str) -> Option<String> {
    for line in out.lines() {
        let lower = line.to_lowercase();
        for key in ["file system name", "文件系统名称"] {
            if let Some(idx) = lower.find(key) {
                let after = &line[idx + key.len()..];
                if let Some(colon) = after.find(':') {
                    let v = after[colon + 1..].trim();
                    if !v.is_empty() {
                        return Some(v.to_string());
                    }
                }
            }
        }
    }
    None
}

/// Get-Volume | ConvertTo-Json 数组里找第一个 FileSystem
fn parse_volume_json(json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    match v {
        serde_json::Value::Array(arr) => arr.iter().find_map(|item| {
            item.get("FileSystem")
                .and_then(|f| f.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        }),
        serde_json::Value::Object(obj) => obj
            .get("FileSystem")
            .and_then(|f| f.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        _ => None,
    }
}

/// 解析逻辑与 IO 分离 —— 独立可测
pub fn parse_project_location(path: &str, filesystem: &str) -> BTreeMap<String, FactValue> {
    let mut facts = BTreeMap::new();

    let drive = path
        .chars()
        .next()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| format!("{c}:"))
        .unwrap_or_default();
    if !drive.is_empty() {
        facts.insert("drive".to_string(), FactValue::Str(drive));
    }
    if !filesystem.is_empty() {
        facts.insert(
            "filesystem".to_string(),
            FactValue::Str(filesystem.to_string()),
        );
    }

    let lower = path.to_lowercase();
    // WSL 路径：\\wsl$\ 或 \\wsl.localhost\
    let on_wsl = lower.starts_with(r"\\wsl$\") || lower.starts_with(r"\\wsl.localhost\");
    // 网络盘：UNC 路径（\\server\share），且不是 WSL
    let on_network = lower.starts_with(r"\\") && !on_wsl;
    // OneDrive：路径含 "onedrive" 或 %OneDrive% 环境变量前缀
    let on_onedrive = lower.contains("onedrive")
        || std::env::var("OneDrive")
            .map(|od| !od.is_empty() && lower.starts_with(&od.to_lowercase()))
            .unwrap_or(false);
    // 非 ASCII（中文等）
    let path_has_non_ascii = !path.is_ascii();
    let path_has_space = path.contains(' ');

    facts.insert("on_onedrive".to_string(), FactValue::Bool(on_onedrive));
    facts.insert("on_network".to_string(), FactValue::Bool(on_network));
    facts.insert("on_wsl".to_string(), FactValue::Bool(on_wsl));
    facts.insert(
        "path_has_non_ascii".to_string(),
        FactValue::Bool(path_has_non_ascii),
    );
    facts.insert(
        "path_has_space".to_string(),
        FactValue::Bool(path_has_space),
    );
    facts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_local_c_drive() {
        let facts = parse_project_location(r"C:\Users\me\proj", "NTFS");
        assert_eq!(
            facts.get("drive").unwrap(),
            &FactValue::Str("C:".to_string())
        );
        assert_eq!(
            facts.get("filesystem").unwrap(),
            &FactValue::Str("NTFS".to_string())
        );
        assert_eq!(facts.get("on_onedrive").unwrap(), &FactValue::Bool(false));
        assert_eq!(facts.get("on_network").unwrap(), &FactValue::Bool(false));
        assert_eq!(facts.get("on_wsl").unwrap(), &FactValue::Bool(false));
        assert_eq!(
            facts.get("path_has_non_ascii").unwrap(),
            &FactValue::Bool(false)
        );
        assert_eq!(
            facts.get("path_has_space").unwrap(),
            &FactValue::Bool(false)
        );
    }

    #[test]
    fn parse_chinese_path_with_space() {
        let facts = parse_project_location(r"D:\uking编程\本境协议", "NTFS");
        assert_eq!(
            facts.get("drive").unwrap(),
            &FactValue::Str("D:".to_string())
        );
        assert_eq!(
            facts.get("path_has_non_ascii").unwrap(),
            &FactValue::Bool(true)
        );
        assert_eq!(
            facts.get("path_has_space").unwrap(),
            &FactValue::Bool(false)
        );
    }

    #[test]
    fn parse_wsl_path() {
        let facts = parse_project_location(r"\\wsl$\Ubuntu\home\dev\proj", "9p");
        assert_eq!(facts.get("on_wsl").unwrap(), &FactValue::Bool(true));
        assert_eq!(facts.get("on_network").unwrap(), &FactValue::Bool(false));
        assert!(!facts.contains_key("drive"));
    }

    #[test]
    fn parse_onedrive_path() {
        let facts = parse_project_location(r"C:\Users\me\OneDrive\Documents\proj", "NTFS");
        assert_eq!(facts.get("on_onedrive").unwrap(), &FactValue::Bool(true));
    }

    #[test]
    fn parse_unc_path() {
        let facts = parse_project_location(r"\\nas\share\proj", "NTFS");
        assert_eq!(facts.get("on_network").unwrap(), &FactValue::Bool(true));
        assert_eq!(facts.get("on_wsl").unwrap(), &FactValue::Bool(false));
    }

    #[test]
    fn parse_fsutil_en_and_zh() {
        assert_eq!(
            parse_fsutil("File System Name : NTFS").as_deref(),
            Some("NTFS")
        );
        assert_eq!(parse_fsutil("文件系统名称 : NTFS").as_deref(), Some("NTFS"));
        assert_eq!(parse_fsutil("nothing here"), None);
    }

    #[test]
    fn parse_volume_json_single_and_array() {
        assert_eq!(
            parse_volume_json(r#"{"FileSystem":"NTFS"}"#).as_deref(),
            Some("NTFS")
        );
        assert_eq!(
            parse_volume_json(r#"[{"FileSystem":"FAT32"},{"FileSystem":"NTFS"}]"#).as_deref(),
            Some("FAT32")
        );
        assert_eq!(parse_volume_json("not json"), None);
    }
}
