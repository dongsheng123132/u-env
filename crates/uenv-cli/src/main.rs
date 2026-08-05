mod cli;
mod output;

use std::collections::BTreeMap;
use std::process;
use std::time::Instant;

use clap::Parser;
use uenv_core::{DetectStatus, DetectorRecord, Environment, EnvironmentIdentity, OperatingSystem};
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
        Commands::Doctor { fail_on: _ } => {
            out.log("uenv doctor: not implemented yet");
            if cli.json {
                out.json(false, None::<()>, Some("not implemented yet".into()), None);
            }
            process::exit(1);
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
