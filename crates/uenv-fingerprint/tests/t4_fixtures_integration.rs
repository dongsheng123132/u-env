// 集成测试：../../fixtures/env-good.json vs env-broken.json（任务书测试 5）。
// 需要真实 fixture 文件，路径相对于 workspace root。

use std::collections::BTreeMap;

use uenv_core::Environment;
use uenv_fingerprint::diff::diff_environments;

fn load(path: &str) -> Environment {
    let content = std::fs::read_to_string(path).expect("fixture 应存在");
    serde_json::from_str(&content).expect("fixture 应可解析")
}

#[test]
fn good_vs_broken_high_risk_five_items() {
    let good = load("../../fixtures/env-good.json");
    let broken = load("../../fixtures/env-broken.json");

    let (high, low, only) = diff_environments(&good, &broken);

    // 任务书要求：diff 输出至少列出 Node 版本差异、两个 cargo、缺 WebView2，
    // 且都归在「高风险差异」。
    assert!(!high.is_empty(), "必须有高风险差异");
    assert!(low.is_empty(), "这些差异都应归为高风险");

    // 1. Node 版本差异
    assert!(
        high.iter()
            .any(|d| d.detector == "toolchain.node" && d.key == "versions"),
        "缺 Node 版本差异: {high:?}"
    );
    // 2. 两个 cargo（cargo_paths 从 1 处变 2 处）
    assert!(
        high.iter()
            .any(|d| d.detector == "toolchain.rust" && d.key == "cargo_paths"),
        "缺双 cargo 差异: {high:?}"
    );
    // 3. 缺 WebView2
    assert!(
        high.iter()
            .any(|d| d.detector == "runtime.webview2" && d.key == "installed"),
        "缺 WebView2 差异: {high:?}"
    );
    // 4. 长路径未开
    assert!(
        high.iter()
            .any(|d| d.detector == "windows.long-paths" && d.key == "enabled"),
        "缺长路径差异: {high:?}"
    );
    // 5. 项目在 OneDrive
    assert!(
        high.iter()
            .any(|d| d.detector == "fs.project-location" && d.key == "on_onedrive"),
        "缺 OneDrive 差异: {high:?}"
    );

    assert!(only.is_empty());
}

#[test]
fn good_vs_broken_renders_text() {
    let good = load("../../fixtures/env-good.json");
    let broken = load("../../fixtures/env-broken.json");
    let (high, low, only) = diff_environments(&good, &broken);
    let text = uenv_fingerprint::diff::render_text(&high, &low, &only);
    assert!(text.contains("高风险差异"));
    assert!(text.contains("toolchain.node"));
    assert!(text.contains("runtime.webview2"));
}

#[test]
fn good_fingerprint_stable_and_differs() {
    let good = load("../../fixtures/env-good.json");
    let broken = load("../../fixtures/env-broken.json");
    let (fp1, _) = uenv_fingerprint::compute_fingerprint(&good).unwrap();
    let (fp2, _) = uenv_fingerprint::compute_fingerprint(&good).unwrap();
    assert_eq!(fp1.full, fp2.full, "同输入两次必须同 hash");

    let (fp_broken, _) = uenv_fingerprint::compute_fingerprint(&broken).unwrap();
    assert_ne!(fp1.full, fp_broken.full, "坏机器指纹必须不同");

    // BTreeMap 仅为保留导入意图的占位
    let _ = BTreeMap::<String, String>::new();
}
