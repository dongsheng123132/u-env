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
        Commands::Fingerprint => {
            out.log("uenv fingerprint: not implemented yet");
            if cli.json {
                out.json(false, None::<()>, Some("not implemented yet".into()), None);
            }
            process::exit(1);
        }
        Commands::Diff { a: _, b: _ } => {
            out.log("uenv diff: not implemented yet");
            if cli.json {
                out.json(false, None::<()>, Some("not implemented yet".into()), None);
            }
            process::exit(1);
        }
    }
}

fn run_scan(cli: &Cli, out: &Output, out_path: Option<&std::path::PathBuf>) {
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
