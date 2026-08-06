// uenv-report — markdown / json 渲染（T6 实现）。
//
// markdown 结构（规格 §7.6 + 任务书）：
//   一句话结论 → 环境概览表 → 按 severity 分组的问题（现象/原因/建议）→ 环境指纹 → 附录（全部 detector summary）
//   evidence 正文不贴，长的折叠进 <details>
// json 结构：规格 §7.3 信封格式 {ok, data:{...}, error, stats}

use uenv_core::{DetectStatus, Environment, Finding, Severity};

/// 渲染 markdown 报告
pub fn render_markdown(
    env: &Environment,
    findings: &[Finding],
    skipped_rules: &[&str],
) -> anyhow::Result<String> {
    let mut out = String::new();

    // 一句话结论
    let error_n = count_sev(findings, Severity::Error);
    let warning_n = count_sev(findings, Severity::Warning);
    let info_n = count_sev(findings, Severity::Info);
    let conclusion = if error_n > 0 {
        format!("**发现 {error_n} 个 Error 级问题**——这台机器的环境无法满足项目要求，先解决下面的 Error 再继续。")
    } else if warning_n > 0 {
        format!("环境基本可用，但有 {warning_n} 个 Warning 值得处理。")
    } else {
        "环境健康，未发现问题。".to_string()
    };
    out.push_str(&format!("# u-env 环境报告\n\n{conclusion}\n\n"));

    // 环境概览表
    out.push_str("## 环境概览\n\n");
    out.push_str("| 项 | 值 |\n|---|---|\n");
    let os = &env.identity.os;
    out.push_str(&format!("| 系统 | {} build {} |\n", os.product_name, os.build));
    out.push_str(&format!("| 架构 | {:?} |\n", env.identity.architecture));
    if let Some(p) = &env.identity.project {
        out.push_str(&format!("| 项目根 | `{}` |\n", p.root));
    }
    out.push_str(&format!("| detector 数 | {} |\n", env.detectors.len()));
    out.push('\n');

    // 按 severity 分组的问题
    out.push_str("## 问题\n\n");
    if findings.is_empty() {
        out.push_str("未发现问题。\n\n");
    }
    for (label, sev) in [("### Error", Severity::Error), ("### Warning", Severity::Warning), ("### Info", Severity::Info)] {
        let items: Vec<&Finding> = findings.iter().filter(|f| f.severity == sev).collect();
        if items.is_empty() {
            continue;
        }
        out.push_str(&format!("{label}\n\n"));
        for f in items {
            out.push_str(&format!("#### `{}` — {}\n\n", f.rule_id, f.title));
            out.push_str(&format!("{}. \n\n", f.description));
            if let Some(fix) = &f.suggested_fix {
                out.push_str(&format!("**建议**（{:?}）：{}\n\n", fix.safety, fix.explain));
                if !fix.commands.is_empty() {
                    out.push_str("```bash\n");
                    for c in &fix.commands {
                        out.push_str(c);
                        out.push('\n');
                    }
                    out.push_str("```\n");
                    out.push_str("<details><summary>回滚</summary>\n\n```bash\n");
                    for r in &fix.rollback {
                        out.push_str(r);
                        out.push('\n');
                    }
                    out.push_str("```\n</details>\n\n");
                }
            }
            if !f.evidence.is_empty() {
                out.push_str("<details><summary>证据</summary>\n\n");
                for e in &f.evidence {
                    let excerpt = if e.excerpt.len() > 200 {
                        format!("{}…", &e.excerpt[..200])
                    } else {
                        e.excerpt.clone()
                    };
                    out.push_str(&format!("- `{}`（{}）：`{}`\n", e.source, format!("{:?}", e.kind).to_lowercase(), excerpt));
                }
                out.push_str("\n</details>\n\n");
            }
        }
    }

    // 指纹
    out.push_str("## 环境指纹\n\n");
    if let Some(fp) = &env.fingerprint {
        out.push_str(&format!("- host: `{}`\n", fp.host));
        out.push_str(&format!("- toolchain: `{}`\n", fp.toolchain));
        if let Some(p) = &fp.project {
            out.push_str(&format!("- project: `{}`\n", p));
        }
        out.push_str(&format!("- full: `{}`\n", fp.full));
    } else {
        out.push_str("（未计算，运行 `uenv fingerprint` 获取）\n");
    }
    out.push('\n');

    // skipped 规则
    if !skipped_rules.is_empty() {
        out.push_str(&format!("> 本次跳过 {} 条不相关规则：{}。\n\n", skipped_rules.len(), skipped_rules.join(", ")));
    }

    // 附录：全部 detector summary
    out.push_str("## 附录：全部 detector\n\n");
    out.push_str("| detector | 状态 | summary |\n|---|---|---|\n");
    for (id, record) in &env.detectors {
        let status = format!("{:?}", record.status).to_lowercase();
        out.push_str(&format!("| `{id}` | {status} | {} |\n", record.summary.replace('|', "\\|")));
    }
    out.push('\n');

    Ok(out)
}

/// 渲染 json 报告（信封格式 §7.3）
pub fn render_json(
    env: &Environment,
    findings: &[Finding],
    skipped_rules: &[&str],
) -> anyhow::Result<String> {
    let error_n = count_sev(findings, Severity::Error);
    let warning_n = count_sev(findings, Severity::Warning);
    let info_n = count_sev(findings, Severity::Info);

    let findings_json: Vec<serde_json::Value> = findings
        .iter()
        .map(|f| {
            let fix = f.suggested_fix.as_ref().map(|s| serde_json::json!({
                "safety": format!("{:?}", s.safety).to_lowercase(),
                "explain": s.explain,
                "commands": s.commands,
                "rollback": s.rollback,
            }));
            serde_json::json!({
                "rule_id": f.rule_id,
                "severity": format!("{:?}", f.severity).to_lowercase(),
                "title": f.title,
                "description": f.description,
                "evidence": f.evidence.iter().map(|e| serde_json::json!({
                    "kind": format!("{:?}", e.kind).to_lowercase(),
                    "source": e.source,
                    "excerpt": e.excerpt,
                })).collect::<Vec<_>>(),
                "suggested_fix": fix,
            })
        })
        .collect();

    let payload = serde_json::json!({
        "spec": env.spec,
        "identity": {
            "host_alias": env.identity.host_alias,
            "os": {
                "product_name": env.identity.os.product_name,
                "version": env.identity.os.version,
                "build": env.identity.os.build,
            },
            "architecture": format!("{:?}", env.identity.architecture).to_lowercase(),
            "project": env.identity.project,
        },
        "fingerprint": env.fingerprint,
        "findings": findings_json,
        "skipped_rules": skipped_rules,
        "summary": {
            "error": error_n,
            "warning": warning_n,
            "info": info_n,
        },
    });

    Ok(serde_json::to_string_pretty(&payload)?)
}

fn count_sev(findings: &[Finding], sev: Severity) -> usize {
    findings.iter().filter(|f| f.severity == sev).count()
}

/// 从 env 收集"有多少 detector 非 ok"（报告概览用）
pub fn non_ok_count(env: &Environment) -> usize {
    env.detectors
        .values()
        .filter(|r| r.status != DetectStatus::Ok && r.status != DetectStatus::Skipped)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use uenv_core::{DetectorRecord, EnvironmentIdentity, OperatingSystem};

    fn fake_env() -> Environment {
        Environment {
            spec: "origin-environment/v0.1".to_string(),
            generated_at: "2026-08-06T00:00:00Z".to_string(),
            uenv_version: "0.0.1".to_string(),
            identity: EnvironmentIdentity {
                host_alias: "<host>".to_string(),
                os: OperatingSystem {
                    family: "windows".to_string(),
                    product_name: "Windows 11 Home".to_string(),
                    product_name_raw: "Windows 10 Home".to_string(),
                    version: "10.0.22631".to_string(),
                    build: 22631,
                    ubr: None,
                    edition: None,
                    display_version: None,
                },
                architecture: uenv_core::Architecture::X64,
                project: None,
            },
            detectors: BTreeMap::from([(
                "windows.long-paths".to_string(),
                DetectorRecord {
                    id: "windows.long-paths".to_string(),
                    layer: uenv_core::Layer::Host,
                    title: "长路径".to_string(),
                    status: DetectStatus::Ok,
                    summary: "已开启".to_string(),
                    facts: BTreeMap::new(),
                    volatile: BTreeMap::new(),
                    evidence: vec![],
                    elapsed_ms: 5,
                },
            )]),
            fingerprint: None,
        }
    }

    #[test]
    fn markdown_renders_sections() {
        let env = fake_env();
        let f = Finding {
            rule_id: "windows.long-paths-disabled".to_string(),
            severity: Severity::Error,
            title: "长路径未开".to_string(),
            description: "node_modules 深路径会爆".to_string(),
            evidence: vec![],
            suggested_fix: None,
        };
        let md = render_markdown(&env, &[f], &[]).unwrap();
        assert!(md.contains("# u-env 环境报告"));
        assert!(md.contains("## 环境概览"));
        assert!(md.contains("## 问题"));
        assert!(md.contains("## 环境指纹"));
        assert!(md.contains("## 附录"));
        assert!(md.contains("windows.long-paths-disabled"));
    }

    #[test]
    fn json_renders_envelope() {
        let env = fake_env();
        let md = render_json(&env, &[], &["node.version-drift"]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&md).unwrap();
        assert_eq!(v["summary"]["error"], 0);
        assert_eq!(v["skipped_rules"][0], "node.version-drift");
        assert_eq!(v["spec"], "origin-environment/v0.1");
    }
}
