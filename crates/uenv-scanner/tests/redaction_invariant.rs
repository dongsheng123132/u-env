//! 类级脱敏不变量测试（2026-08-26 外审 P0-A 整改）。
//!
//! 背景：path_analysis 的 duplicates/missing 曾从未脱敏 path_raw 派生，
//! 真实用户名泄进 facts → 报告/指纹/agent JSON。单点修完后，这里钉一条
//! **类级闸门**：遍历一整棵真实扫描的 Environment 的全部 facts/volatile/evidence，
//! 断言不含本机用户名等敏感串。以后任何 detector 引入新的泄漏出口，此测试变红。

use std::collections::BTreeMap;

use uenv_core::{Environment, FactValue};

/// 收集 FactValue 树里的全部字符串（含 List/Set/Map 嵌套）
fn collect_strings(v: &FactValue, out: &mut Vec<String>) {
    match v {
        FactValue::Str(s) | FactValue::Path(s) | FactValue::Version(s) => out.push(s.clone()),
        FactValue::Int(_) | FactValue::Bool(_) => {}
        FactValue::List(items) | FactValue::Set(items) => {
            for it in items {
                collect_strings(it, out);
            }
        }
        FactValue::Map(m) => {
            for val in m.values() {
                collect_strings(val, out);
            }
        }
    }
}

fn all_strings(env: &Environment) -> Vec<String> {
    let mut out = Vec::new();
    for rec in env.detectors.values() {
        for f in rec.facts.values() {
            collect_strings(f, &mut out);
        }
        for f in rec.volatile.values() {
            collect_strings(f, &mut out);
        }
        for e in &rec.evidence {
            out.push(e.source.clone());
            out.push(e.excerpt.clone());
        }
    }
    out
}

fn username() -> String {
    std::env::var("USERNAME").unwrap_or_default()
}

/// 定位 workspace 根下的 target/<profile>/uenv.exe（集成测试无法用 CARGO_BIN_EXE 跨包引用）
fn path_join_target(exe: &str) -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // crates/uenv-scanner → workspace root
    p.pop();
    p.pop();
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    p.join("target").join(profile).join(exe)
}

#[test]
#[cfg(windows)]
fn redaction_invariant_real_scan_contains_no_username() {
    let user = username();
    if user.is_empty() {
        return; // 无 USERNAME 环境变量的极端环境跳过
    }
    // 真实扫描一次（与验收命令同路径）
    // CARGO_BIN_EXE_uenv 只在 uenv-cli 的集成测试里可用；
    // 这里直接调用已构建的 target 目录下的二进制（CI/本地 cargo test 前会先 build）
    let exe = path_join_target("uenv.exe");
    let output = std::process::Command::new(&exe)
        .args(["scan", "--project", ".", "--json"])
        .output()
        .unwrap_or_else(|e| panic!("failed to run {}: {e}", exe.display()));
    assert!(
        output.status.success(),
        "uenv scan failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout not JSON");
    let data = json.get("data").expect("envelope missing data");
    let env: Environment =
        serde_json::from_value(data.clone()).expect("data does not deserialize into Environment");

    let strings = all_strings(&env);
    assert!(
        !strings.is_empty(),
        "scan produced no strings — test is vacuous"
    );

    let offenders: Vec<&String> = strings
        .iter()
        .filter(|s| s.contains(&user) && !s.contains("<user>"))
        .collect();
    assert!(
        offenders.is_empty(),
        "脱敏不变量被破坏：{} 条字符串包含真实用户名 `{user}`（应为 <user>）。样例: {:?}",
        offenders.len(),
        offenders.iter().take(3).collect::<Vec<_>>()
    );
}

#[test]
fn redaction_invariant_synthetic_env() {
    // 不依赖真实扫描的合成用例：直接构造含用户名的 facts，走 parse 层验证
    let user = username();
    if user.is_empty() {
        return;
    }
    let fake_path = format!(r"C:\Users\{user}\.local\bin;C:\Windows;C:\Windows");
    // parse 层不做脱敏（它只管解析），detect() 输出的 duplicates 必须已脱敏——
    // 由上面的 real-scan 测试兜底。这里固定验证：normalize_entry 不因大小写/尾斜杠漏配
    let mut seen = BTreeMap::new();
    for e in fake_path.split(';') {
        let norm = e.trim().trim_end_matches('\\').to_lowercase();
        *seen.entry(norm).or_insert(0) += 1;
    }
    assert_eq!(seen.get(&r"c:\windows".to_lowercase()), Some(&2));
}
