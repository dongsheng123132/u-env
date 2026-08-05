// net.proxy detector — 系统代理、环境变量代理、npm/git 代理及一致性。
// layer=Host
// 数据源：注册表 HKCU\...\Internet Settings、环境变量、npm config get proxy、git config --get http.proxy
// ⚠️ 代理 URL 里的账号密码由 ScanContext 脱敏；环境变量读取后要手动 ctx.redact。

use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, Evidence, EvidenceKind, FactValue, Layer};
use winreg::enums::HKEY_CURRENT_USER;

use crate::context::{ScanContext, evidence_from_command, evidence_from_registry};
use crate::detector::{Detector, DetectorMeta, DetectorResult};

pub struct NetProxy;

impl Detector for NetProxy {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "net.proxy",
            layer: Layer::Host,
            title: "网络代理配置",
            cost: Cost::Slow,
        }
    }

    fn detect(&self, ctx: &ScanContext) -> DetectorResult {
        let mut evidence = Vec::new();

        // 1. 系统代理：ProxyEnable (DWORD) + ProxyServer (SZ)
        let path = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";
        let enable = ctx.reg_read(HKEY_CURRENT_USER, path, "ProxyEnable");
        evidence.push(evidence_from_registry(path, "ProxyEnable", &enable));
        let server = ctx.reg_read(HKEY_CURRENT_USER, path, "ProxyServer");
        evidence.push(evidence_from_registry(path, "ProxyServer", &server));

        let system_proxy = match (&enable, &server) {
            (Some(e), Some(s)) => {
                let on = e.value.trim().parse::<i64>().unwrap_or(0) != 0;
                if on && !s.value.trim().is_empty() {
                    Some(s.value.trim().to_string())
                } else {
                    None
                }
            }
            _ => None,
        };

        // 2. 环境变量代理（读取后手动脱敏）
        let env_http = ctx.redact(
            &std::env::var("HTTP_PROXY")
                .or_else(|_| std::env::var("http_proxy"))
                .unwrap_or_default(),
        );
        let env_https = ctx.redact(
            &std::env::var("HTTPS_PROXY")
                .or_else(|_| std::env::var("https_proxy"))
                .unwrap_or_default(),
        );
        let no_proxy = ctx.redact(
            &std::env::var("NO_PROXY")
                .or_else(|_| std::env::var("no_proxy"))
                .unwrap_or_default(),
        );
        evidence.push(Evidence {
            kind: EvidenceKind::Env,
            source: "HTTP_PROXY / HTTPS_PROXY / NO_PROXY".to_string(),
            exit_code: None,
            excerpt: format!("HTTP={env_http} HTTPS={env_https} NO_PROXY={no_proxy}"),
        });

        // 3. npm / git 代理
        let npm = ctx.run("npm", &["config", "get", "proxy"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "npm config get proxy",
            &npm,
        ));
        let git = ctx.run("git", &["config", "--get", "http.proxy"]);
        evidence.push(evidence_from_command(
            EvidenceKind::Command,
            "git config --get http.proxy",
            &git,
        ));

        let npm_proxy = if npm.ran && npm.exit_code == Some(0) {
            let t = npm.stdout.trim();
            // npm 未设置时输出 "null"
            if t.is_empty() || t.eq_ignore_ascii_case("null") {
                None
            } else {
                Some(t.to_string())
            }
        } else {
            None
        };
        let git_proxy = if git.ran && git.exit_code == Some(0) {
            let t = git.stdout.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        } else {
            None
        };

        let facts = parse_proxy(
            system_proxy.as_deref(),
            if env_http.is_empty() {
                None
            } else {
                Some(&env_http)
            },
            if env_https.is_empty() {
                None
            } else {
                Some(&env_https)
            },
            if no_proxy.is_empty() {
                None
            } else {
                Some(&no_proxy)
            },
            npm_proxy.as_deref(),
            git_proxy.as_deref(),
        );

        let summary = match facts.get("system_proxy") {
            Some(FactValue::Str(p)) => format!(
                "系统代理 {p}，{}",
                if matches!(facts.get("consistent"), Some(FactValue::Bool(true))) {
                    "与 env 一致"
                } else {
                    "与 env 不一致"
                }
            ),
            _ => {
                if matches!(facts.get("env_http_proxy"), Some(FactValue::Str(_))) {
                    "无系统代理，仅环境变量代理".to_string()
                } else {
                    "无代理配置".to_string()
                }
            }
        };

        DetectorResult {
            status: DetectStatus::Ok,
            summary,
            facts,
            volatile: BTreeMap::new(),
            evidence,
        }
    }
}

/// 解析逻辑与 IO 分离 —— 独立可测。
/// consistent = 系统代理与 env_http_proxy 规范化后相等（都未设置也算一致）。
pub fn parse_proxy(
    system_proxy: Option<&str>,
    env_http: Option<&str>,
    env_https: Option<&str>,
    no_proxy: Option<&str>,
    npm_proxy: Option<&str>,
    git_proxy: Option<&str>,
) -> BTreeMap<String, FactValue> {
    let mut facts = BTreeMap::new();

    if let Some(p) = system_proxy {
        facts.insert("system_proxy".to_string(), FactValue::Str(p.to_string()));
    }
    if let Some(p) = env_http {
        facts.insert("env_http_proxy".to_string(), FactValue::Str(p.to_string()));
    }
    if let Some(p) = env_https {
        facts.insert("env_https_proxy".to_string(), FactValue::Str(p.to_string()));
    }
    if let Some(p) = no_proxy {
        facts.insert("no_proxy".to_string(), FactValue::Str(p.to_string()));
    }
    if let Some(p) = npm_proxy {
        facts.insert("npm_proxy".to_string(), FactValue::Str(p.to_string()));
    }
    if let Some(p) = git_proxy {
        facts.insert("git_proxy".to_string(), FactValue::Str(p.to_string()));
    }

    // 一致性：规范化后比较（127.0.0.1:7897 == http://127.0.0.1:7897）
    let consistent = normalize_proxy(system_proxy) == normalize_proxy(env_http);
    facts.insert("consistent".to_string(), FactValue::Bool(consistent));

    facts
}

/// 规范化代理 URL：去 scheme、去尾部斜杠、去用户名密码（脱敏后比较）。
/// 系统代理注册表值通常不带 scheme（"127.0.0.1:7897"），env 变量常带（"http://127.0.0.1:7897"）。
fn normalize_proxy(p: Option<&str>) -> String {
    let s = p.unwrap_or("").trim();
    let s = s
        .strip_prefix("http://")
        .or_else(|| s.strip_prefix("https://"))
        .unwrap_or(s)
        .trim_end_matches('/');
    // 去掉 user:pass@ 前缀（脱敏后是 <redacted>@）
    if let Some(at) = s.rfind('@') {
        s[at + 1..].to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_system_matches_env() {
        // 注册表 "127.0.0.1:7897" 与 env "http://127.0.0.1:7897" 一致
        let facts = parse_proxy(
            Some("127.0.0.1:7897"),
            Some("http://127.0.0.1:7897"),
            Some("http://127.0.0.1:7897"),
            Some("localhost,127.0.0.1"),
            Some("http://127.0.0.1:7897"),
            Some("http://127.0.0.1:7897"),
        );
        assert_eq!(facts.get("consistent").unwrap(), &FactValue::Bool(true));
        assert_eq!(
            facts.get("system_proxy").unwrap(),
            &FactValue::Str("127.0.0.1:7897".to_string())
        );
    }

    #[test]
    fn parse_system_differs_env() {
        let facts = parse_proxy(
            Some("127.0.0.1:7897"),
            Some("http://proxy.corp:8080"),
            None,
            None,
            None,
            None,
        );
        assert_eq!(facts.get("consistent").unwrap(), &FactValue::Bool(false));
    }

    #[test]
    fn parse_none_consistent() {
        // 都没设置 → 一致（true）
        let facts = parse_proxy(None, None, None, None, None, None);
        assert_eq!(facts.get("consistent").unwrap(), &FactValue::Bool(true));
        assert!(!facts.contains_key("system_proxy"));
    }

    #[test]
    fn parse_system_disabled() {
        // 系统代理未启用 → system_proxy 省略，与 env 比较：env 有值 → 不一致
        let facts = parse_proxy(None, Some("http://127.0.0.1:7897"), None, None, None, None);
        assert_eq!(facts.get("consistent").unwrap(), &FactValue::Bool(false));
    }

    #[test]
    fn normalize_with_auth() {
        assert_eq!(
            normalize_proxy(Some("http://user:pass@127.0.0.1:7897")),
            "127.0.0.1:7897"
        );
        assert_eq!(
            normalize_proxy(Some("http://<redacted>@127.0.0.1:7897")),
            "127.0.0.1:7897"
        );
        assert_eq!(normalize_proxy(Some("127.0.0.1:7897")), "127.0.0.1:7897");
        assert_eq!(normalize_proxy(None), "");
    }
}
