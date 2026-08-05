// toolchain.npm-family detector — npm/pnpm/yarn/bun/corepack 版本、路径、npm prefix/registry。
// layer=Toolchain
// ⚠️ npm 在 Windows 上启动慢 → 用 ctx.run_slow (20s)。

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, EvidenceKind, FactValue, Layer};

use crate::context::{evidence_from_command, ScanContext};
use crate::detector::{Detector, DetectorMeta, DetectorResult};

pub struct ToolchainNpmFamily;

impl Detector for ToolchainNpmFamily {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "toolchain.npm-family",
            layer: Layer::Toolchain,
            title: "npm 家族",
            cost: Cost::Slow,
        }
    }

    fn detect(&self, ctx: &ScanContext) -> DetectorResult {
        let mut evidence = Vec::new();

        // npm 版本（slow：npm 启动慢）
        let npm_ver = ctx.run_slow("npm", &["--version"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "npm --version",
            &npm_ver,
        ));
        // prefix / registry（慢命令，合并跑减少启动次数：npm config get prefix && npm config get registry）
        let npm_prefix = ctx.run_slow("npm", &["config", "get", "prefix"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "npm config get prefix",
            &npm_prefix,
        ));
        let npm_registry = ctx.run_slow("npm", &["config", "get", "registry"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "npm config get registry",
            &npm_registry,
        ));

        // 版本探测（快速 --version）
        let pnpm_ver = ctx.run("pnpm", &["--version"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "pnpm --version",
            &pnpm_ver,
        ));
        let yarn_ver = ctx.run("yarn", &["--version"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "yarn --version",
            &yarn_ver,
        ));
        let bun_ver = ctx.run("bun", &["--version"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "bun --version",
            &bun_ver,
        ));
        let corepack_ver = ctx.run("corepack", &["--version"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "corepack --version",
            &corepack_ver,
        ));

        let facts = parse_npm_family(
            &npm_ver.stdout,
            npm_ver.ran,
            &npm_prefix.stdout,
            npm_prefix.ran,
            &npm_registry.stdout,
            npm_registry.ran,
            &pnpm_ver.stdout,
            pnpm_ver.ran,
            &yarn_ver.stdout,
            yarn_ver.ran,
            &bun_ver.stdout,
            bun_ver.ran,
            &corepack_ver.stdout,
            corepack_ver.ran,
        );

        let (status, summary) = if facts.is_empty() {
            (
                DetectStatus::Absent,
                "npm 家族工具未安装".to_string(),
            )
        } else {
            let npm_v = facts
                .get("npm_version")
                .map(|v| match v {
                    FactValue::Version(s) => s.clone(),
                    _ => String::new(),
                })
                .unwrap_or_default();
            (
                DetectStatus::Ok,
                if npm_v.is_empty() {
                    "npm 家族部分安装".to_string()
                } else {
                    format!("npm {npm_v}")
                },
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
#[allow(clippy::too_many_arguments)]
pub fn parse_npm_family(
    npm_ver: &str,
    npm_ran: bool,
    npm_prefix: &str,
    prefix_ran: bool,
    npm_registry: &str,
    registry_ran: bool,
    pnpm_ver: &str,
    pnpm_ran: bool,
    yarn_ver: &str,
    yarn_ran: bool,
    bun_ver: &str,
    bun_ran: bool,
    corepack_ver: &str,
    corepack_ran: bool,
) -> BTreeMap<String, FactValue> {
    let mut facts = BTreeMap::new();

    if npm_ran {
        let v = npm_ver.trim();
        if !v.is_empty() {
            facts.insert("npm_version".to_string(), FactValue::Version(v.to_string()));
        }
    }
    if prefix_ran {
        let p = npm_prefix.trim();
        // npm config get prefix 未设置时输出 null
        if !p.is_empty() && !p.eq_ignore_ascii_case("null") {
            facts.insert("npm_prefix".to_string(), FactValue::Path(p.to_string()));
        }
    }
    if registry_ran {
        let r = npm_registry.trim();
        if !r.is_empty() && !r.eq_ignore_ascii_case("null") {
            facts.insert("npm_registry".to_string(), FactValue::Str(r.to_string()));
        }
    }
    if pnpm_ran {
        let v = pnpm_ver.trim();
        if !v.is_empty() {
            facts.insert("pnpm_version".to_string(), FactValue::Version(v.to_string()));
        }
    }
    if yarn_ran {
        let v = yarn_ver.trim();
        if !v.is_empty() {
            facts.insert("yarn_version".to_string(), FactValue::Version(v.to_string()));
        }
    }
    if bun_ran {
        let v = bun_ver.trim();
        if !v.is_empty() {
            facts.insert("bun_version".to_string(), FactValue::Version(v.to_string()));
        }
    }
    if corepack_ran {
        let v = corepack_ver.trim();
        if !v.is_empty() {
            facts.insert(
                "corepack_enabled".to_string(),
                FactValue::Bool(!v.eq_ignore_ascii_case("null")),
            );
        }
    }

    facts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_install() {
        let facts = parse_npm_family(
            "10.9.2", true,
            r"C:\Users\me\AppData\Roaming\npm", true,
            "https://registry.npmmirror.com", true,
            "10.33.0", true,
            "1.22.21", true,
            "", false, // bun 未装
            "0.31.0", true,
        );
        assert_eq!(
            facts.get("npm_version").unwrap(),
            &FactValue::Version("10.9.2".to_string())
        );
        assert_eq!(
            facts.get("npm_prefix").unwrap(),
            &FactValue::Path(r"C:\Users\me\AppData\Roaming\npm".to_string())
        );
        assert_eq!(
            facts.get("pnpm_version").unwrap(),
            &FactValue::Version("10.33.0".to_string())
        );
        assert_eq!(
            facts.get("yarn_version").unwrap(),
            &FactValue::Version("1.22.21".to_string())
        );
        assert!(!facts.contains_key("bun_version"));
        assert_eq!(
            facts.get("corepack_enabled").unwrap(),
            &FactValue::Bool(true)
        );
    }

    #[test]
    fn parse_npm_null_config() {
        // npm config get prefix 输出 null（未设置）→ 不进 facts
        let facts = parse_npm_family(
            "10.9.2", true,
            "null", true,
            "null", true,
            "", false, "", false, "", false, "", false,
        );
        assert!(!facts.contains_key("npm_prefix"));
        assert!(!facts.contains_key("npm_registry"));
    }

    #[test]
    fn parse_garbage_input() {
        // 畸形输入：不 panic
        let facts = parse_npm_family(
            "\r\n  ", true, "", false, "", false, "", false, "", false, "", false, "", false,
        );
        assert!(!facts.contains_key("npm_version"));
    }

    #[test]
    fn parse_not_installed() {
        let facts = parse_npm_family("", false, "", false, "", false, "", false, "", false, "", false, "", false);
        assert!(facts.is_empty());
    }
}
