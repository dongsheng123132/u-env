// project.lockfiles detector — 各 lockfile 是否存在及其 sha256。
// layer=Project
// ⚠️ lockfile 可能很大：只算 sha256，不把内容读进 facts 或 evidence（硬规则）。

use std::collections::BTreeMap;
use std::io::Read;

use uenv_core::{Cost, DetectStatus, Evidence, EvidenceKind, FactValue, Layer};

use crate::context::ScanContext;
use crate::detector::{Detector, DetectorMeta, DetectorResult};

/// 常见 lockfile 清单
const LOCKFILES: &[&str] = &[
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "bun.lock",
    "bun.lockb",
    "Cargo.lock",
    "poetry.lock",
    "uv.lock",
    "composer.lock",
    "requirements.lock",
];

pub struct ProjectLockfiles;

impl Detector for ProjectLockfiles {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "project.lockfiles",
            layer: Layer::Project,
            title: "锁文件",
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

        let mut lockfiles: BTreeMap<String, String> = BTreeMap::new();
        let mut evidence = Vec::new();

        for name in LOCKFILES {
            let path = root.join(name);
            if !path.is_file() {
                continue;
            }
            match sha256_file(&path) {
                Some(hash) => {
                    lockfiles.insert(name.to_string(), hash.clone());
                    evidence.push(Evidence {
                        kind: EvidenceKind::File,
                        source: name.to_string(),
                        exit_code: None,
                        excerpt: format!("sha256:{hash}"), // 只放 hash，不放内容
                    });
                }
                None => {
                    evidence.push(Evidence {
                        kind: EvidenceKind::File,
                        source: name.to_string(),
                        exit_code: None,
                        excerpt: "(sha256 计算失败)".to_string(),
                    });
                }
            }
        }

        let (status, summary) = if lockfiles.is_empty() {
            (DetectStatus::Ok, "未发现 lockfile".to_string())
        } else {
            (DetectStatus::Ok, format!("{} 个 lockfile", lockfiles.len()))
        };

        let mut facts = BTreeMap::new();
        if !lockfiles.is_empty() {
            let m: BTreeMap<String, FactValue> = lockfiles
                .into_iter()
                .map(|(k, v)| (k, FactValue::Str(v)))
                .collect();
            facts.insert("lockfiles".to_string(), FactValue::Map(m));
        }

        DetectorResult {
            status,
            summary,
            facts,
            volatile: BTreeMap::new(),
            evidence,
        }
    }
}

/// 计算文件 sha256（hex 小写）—— 与 IO 分离，独立可测（喂字节）
pub fn sha256_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in digest {
        hex.push_str(&format!("{b:02x}"));
    }
    hex
}

fn sha256_file(path: &std::path::Path) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    Some(sha256_bytes(&buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_value() {
        // "hello" 的 sha256
        let hash = sha256_bytes(b"hello");
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn sha256_empty() {
        let hash = sha256_bytes(b"");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_stable() {
        // 同输入两次 → 同 hash
        assert_eq!(sha256_bytes(b"abc"), sha256_bytes(b"abc"));
    }
}
