mod cli;
mod output;

use std::collections::BTreeMap;
use std::process;
use std::time::Instant;

use clap::Parser;
use uenv_core::{
    DetectStatus, DetectorRecord, Environment, EnvironmentIdentity, Finding, OperatingSystem,
    Severity,
};
use uenv_scanner::context::ScanContext;
use uenv_scanner::registry::all_detectors;

use crate::cli::{Cli, Commands};
use crate::output::{Output, OutputStats};

fn main() {
    let cli = Cli::parse();
    let out = Output::new(cli.json, cli.quiet, cli.verbose);

    match &cli.command {
        Commands::Scan { out: out_path } => {
            run_scan(&cli, &out, out_path.as_ref());
        }
        Commands::Doctor {
            fail_on,
            from,
            agent,
        } => {
            run_doctor(&cli, &out, fail_on, from.as_ref(), *agent);
        }
        Commands::Report { format: _, out: _ } => {
            out.log("uenv report: not implemented yet");
            if cli.json {
                out.json(false, None::<()>, Some("not implemented yet".into()), None);
            }
            process::exit(1);
        }
        Commands::Fingerprint { from } => {
            run_fingerprint(&cli, &out, from.as_ref());
        }
        Commands::Diff { a, b } => {
            run_diff(&out, a, b);
        }
    }
}

/// uenv doctor [--project .] [--json] [--fail-on none|warning|error] [--agent] [--from <json>]
fn run_doctor(
    cli: &Cli,
    out: &Output,
    fail_on: &str,
    from: Option<&std::path::PathBuf>,
    agent: bool,
) {
    let env = if let Some(path) = from {
        match load_env(path, out) {
            Some(e) => e,
            None => process::exit(1),
        }
    } else {
        let (env, _count, _elapsed) = run_scan_env(cli, out);
        env
    };

    // 匹配的 adapter（项目类型识别）
    let mut relevant: std::collections::BTreeSet<&'static str> = std::collections::BTreeSet::new();
    for adapter in uenv_adapters::all_adapters() {
        if adapter.matches(&env) {
            for d in adapter.meta().relevant_detectors {
                relevant.insert(d);
            }
        }
    }

    // 跑规则：relevant 非空时只跑相关规则，不相关标记 skipped；relevant 空（非项目）全跑
    let mut findings: Vec<Finding> = Vec::new();
    let mut skipped: Vec<&'static str> = Vec::new();

    for rule in uenv_rules::all_rules() {
        let rule_rel = rule.relevant_detectors();
        let related = if relevant.is_empty() {
            true // 无项目类型匹配 → 全跑（如只扫机器不扫项目）
        } else {
            rule_rel.is_empty() || rule_rel.iter().any(|d| relevant.contains(d))
        };
        if !related {
            skipped.push(rule.id());
            continue;
        }
        findings.extend(rule.evaluate(&env));
    }

    // 统计
    let error_n = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let warning_n = findings
        .iter()
        .filter(|f| f.severity == Severity::Warning)
        .count();
    let info_n = findings
        .iter()
        .filter(|f| f.severity == Severity::Info)
        .count();

    // agent 模式：精简输出，去掉 evidence 正文（保留 source）
    let json_mode = out.json_mode || agent;
    if json_mode {
        let payload = serde_json::json!({
            "findings": findings.iter().map(|f| finding_to_json(f, agent)).collect::<Vec<_>>(),
            "skipped_rules": skipped,
            "summary": { "error": error_n, "warning": warning_n, "info": info_n },
        });
        out.json(true, Some(&payload), None, None);
    } else {
        render_doctor_text(out, &findings, &skipped, error_n, warning_n, info_n);
    }

    // 退出码：默认 --fail-on error
    let fail_level = match fail_on {
        "none" => 0,
        "warning" => {
            if error_n > 0 || warning_n > 0 {
                1
            } else {
                0
            }
        }
        _ => {
            if error_n > 0 {
                1
            } else {
                0
            }
        }
    };
    if fail_level == 1 {
        process::exit(1);
    }
}

fn finding_to_json(f: &Finding, agent: bool) -> serde_json::Value {
    let fix = f.suggested_fix.as_ref().map(|s| {
        serde_json::json!({
            "safety": format!("{:?}", s.safety).to_lowercase(),
            "explain": s.explain,
            "commands": s.commands,
            "rollback": s.rollback,
        })
    });
    serde_json::json!({
        "rule_id": f.rule_id,
        "severity": format!("{:?}", f.severity).to_lowercase(),
        "title": f.title,
        "description": f.description,
        // agent 模式去掉 evidence 正文，保留 source
        "evidence_sources": if agent {
            serde_json::Value::Array(
                f.evidence.iter().map(|e| serde_json::json!({"source": e.source})).collect(),
            )
        } else {
            serde_json::Value::Array(
                f.evidence.iter().map(|e| serde_json::json!({
                    "kind": format!("{:?}", e.kind).to_lowercase(),
                    "source": e.source,
                    "excerpt": e.excerpt,
                })).collect(),
            )
        },
        "suggested_fix": fix,
    })
}

fn render_doctor_text(
    out: &Output,
    findings: &[Finding],
    skipped: &[&'static str],
    error_n: usize,
    warning_n: usize,
    info_n: usize,
) {
    if error_n > 0 {
        out.text(&format!("\n[Error] {error_n} 条："));
        for f in findings.iter().filter(|f| f.severity == Severity::Error) {
            render_finding_text(out, f);
        }
    }
    if warning_n > 0 {
        out.text(&format!("\n[Warning] {warning_n} 条："));
        for f in findings.iter().filter(|f| f.severity == Severity::Warning) {
            render_finding_text(out, f);
        }
    }
    if info_n > 0 {
        out.text(&format!("\n[Info] {info_n} 条："));
        for f in findings.iter().filter(|f| f.severity == Severity::Info) {
            render_finding_text(out, f);
        }
    }
    if !skipped.is_empty() {
        out.text(&format!(
            "\n[skipped] {} 条规则未运行：{}",
            skipped.len(),
            skipped.join(", ")
        ));
    }
    out.text(&format!(
        "\n总结：{error_n} error / {warning_n} warning / {info_n} info"
    ));
}

fn render_finding_text(out: &Output, f: &Finding) {
    out.text(&format!("  [{}({:?})] {}", f.rule_id, f.severity, f.title));
    out.text(&format!("    现象/原因：{}", f.description));
    if let Some(fix) = &f.suggested_fix {
        out.text(&format!("    建议（{:?}）：{}", fix.safety, fix.explain));
        for c in &fix.commands {
            out.text(&format!("      $ {c}"));
        }
    }
}

/// uenv fingerprint [--from <json>] [--json]
fn run_fingerprint(cli: &Cli, out: &Output, from: Option<&std::path::PathBuf>) {
    let env = if let Some(path) = from {
        // 从快照文件读
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                out.log(&format!("failed to read {}: {e}", path.display()));
                process::exit(1);
            }
        };
        let env: Environment = match serde_json::from_str(&content) {
            Ok(e) => e,
            Err(e) => {
                out.log(&format!("failed to parse {}: {e}", path.display()));
                process::exit(1);
            }
        };
        // 硬规则：spec 不一致时明确报错退出 1
        if env.spec != "origin-environment/v0.1" {
            out.log(&format!(
                "spec mismatch: file says {:?}, this build expects {:?}",
                env.spec, "origin-environment/v0.1"
            ));
            process::exit(1);
        }
        env
    } else {
        // 现场 scan 后计算
        out.log("u-env fingerprint: scanning first...");
        let (env, _count, _elapsed) = run_scan_env(cli, out);
        env
    };

    let (fp, excluded) = match compute(&env, out) {
        Some(v) => v,
        None => return,
    };

    if !excluded.is_empty() {
        out.log(&format!(
            "⚠️  本次指纹缺少 {} 个 detector（status=Error）：{}",
            excluded.len(),
            excluded.join(", ")
        ));
    }

    if out.json_mode {
        out.json(
            true,
            Some(&serde_json::json!({
                "host": fp.host,
                "toolchain": fp.toolchain,
                "project": fp.project,
                "full": fp.full,
                "short": uenv_fingerprint::short(&fp),
                "excluded_detectors": excluded,
            })),
            None,
            None,
        );
    } else {
        out.text(&format!("host:      {}", fp.host));
        out.text(&format!("toolchain: {}", fp.toolchain));
        if let Some(p) = &fp.project {
            out.text(&format!("project:   {p}"));
        }
        out.text(&format!("full:      {}", fp.full));
        out.text(&format!("short:     {}", uenv_fingerprint::short(&fp)));
        if !excluded.is_empty() {
            out.text(&format!(
                "⚠️  缺少 {} 个 detector：{}",
                excluded.len(),
                excluded.join(", ")
            ));
        }
    }
}

fn compute(
    env: &Environment,
    out: &Output,
) -> Option<(uenv_core::EnvironmentFingerprint, Vec<String>)> {
    match uenv_fingerprint::compute_fingerprint(env) {
        Ok(v) => Some(v),
        Err(e) => {
            out.log(&format!("fingerprint error: {e}"));
            None
        }
    }
}

/// uenv diff <a.json> <b.json> [--json]
fn run_diff(out: &Output, a: &std::path::PathBuf, b: &std::path::PathBuf) {
    let env_a = match load_env(a, out) {
        Some(e) => e,
        None => process::exit(1),
    };
    let env_b = match load_env(b, out) {
        Some(e) => e,
        None => process::exit(1),
    };

    let (high, low, only) = uenv_fingerprint::diff_environments(&env_a, &env_b);

    if out.json_mode {
        out.json(
            true,
            Some(&uenv_fingerprint::render_json(&high, &low, &only)),
            None,
            None,
        );
    } else {
        out.text(&uenv_fingerprint::render_text(&high, &low, &only));
    }
}

fn load_env(path: &std::path::PathBuf, out: &Output) -> Option<Environment> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            out.log(&format!("failed to read {}: {e}", path.display()));
            return None;
        }
    };
    let env: Environment = match serde_json::from_str(&content) {
        Ok(e) => e,
        Err(e) => {
            out.log(&format!("failed to parse {}: {e}", path.display()));
            return None;
        }
    };
    if env.spec != "origin-environment/v0.1" {
        out.log(&format!(
            "spec mismatch in {}: {:?} != origin-environment/v0.1",
            path.display(),
            env.spec
        ));
        return None;
    }
    Some(env)
}

fn run_scan(cli: &Cli, out: &Output, out_path: Option<&std::path::PathBuf>) {
    let (env, count, total_elapsed) = run_scan_env(cli, out);

    // 输出
    if out.json_mode {
        out.json(
            true,
            Some(&env),
            None,
            Some(OutputStats {
                elapsed_ms: total_elapsed,
                count,
            }),
        );
    } else if let Some(path) = out_path {
        let json = serde_json::to_string_pretty(&env).unwrap_or_default();
        if let Err(e) = std::fs::write(path, &json) {
            out.log(&format!("Failed to write output: {e}"));
            process::exit(1);
        }
        out.log(&format!("Environment written to {}", path.display()));
    } else {
        // 非 JSON 非 out：输出友好的摘要
        for (id, record) in &env.detectors {
            out.text(&format!(
                "[{:?}] {} — {:?}: {}",
                record.layer, id, record.status, record.summary
            ));
        }
        out.text(&format!("\nTotal: {count} detectors in {total_elapsed}ms"));
    }
}

/// 执行一次完整 scan，返回 (Environment, detector 数, 总耗时)
fn run_scan_env(cli: &Cli, out: &Output) -> (Environment, usize, u64) {
    let start = Instant::now();

    // 构建 ScanContext
    let project_root = cli.project.clone();
    let ctx = ScanContext {
        project_root: project_root.clone(),
        redact: !cli.no_redact,
        ..Default::default()
    };

    out.log("u-env scan starting...");

    // 获取所有 detector 并执行
    let detectors = all_detectors();
    let mut records = BTreeMap::new();
    let count = detectors.len();

    for det in &detectors {
        let meta = det.meta();
        out.log(&format!(
            "  running {} (layer={:?})...",
            meta.id, meta.layer
        ));

        if !det.applicable(&ctx) {
            records.insert(
                meta.id.to_string(),
                DetectorRecord {
                    id: meta.id.to_string(),
                    layer: meta.layer,
                    title: meta.title.to_string(),
                    status: DetectStatus::Skipped,
                    summary: "Not applicable".to_string(),
                    facts: BTreeMap::new(),
                    volatile: BTreeMap::new(),
                    evidence: vec![],
                    elapsed_ms: 0,
                },
            );
            continue;
        }

        let det_start = Instant::now();
        let result = det.detect(&ctx);
        let elapsed = det_start.elapsed().as_millis() as u64;

        records.insert(
            meta.id.to_string(),
            DetectorRecord {
                id: meta.id.to_string(),
                layer: meta.layer,
                title: meta.title.to_string(),
                status: result.status,
                summary: result.summary,
                facts: result.facts,
                volatile: result.volatile,
                evidence: result.evidence,
                elapsed_ms: elapsed,
            },
        );
    }

    let total_elapsed = start.elapsed().as_millis() as u64;

    // 构建环境对象
    let env = Environment {
        spec: "origin-environment/v0.1".to_string(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        uenv_version: env!("CARGO_PKG_VERSION").to_string(),
        identity: EnvironmentIdentity {
            host_alias: "<host>".to_string(),
            os: OperatingSystem {
                family: "windows".to_string(),
                product_name: "Unknown".to_string(),
                product_name_raw: "Unknown".to_string(),
                version: "0.0.0".to_string(),
                build: 0,
                ubr: None,
                edition: None,
                display_version: None,
            },
            architecture: uenv_core::Architecture::X64,
            project: None,
        },
        detectors: records,
        fingerprint: None,
    };

    out.log(&format!(
        "scan complete: {count} detectors in {total_elapsed}ms"
    ));

    (env, count, total_elapsed)
}
