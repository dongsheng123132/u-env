// project.kind detector — 项目类型判定（可多选）。
// layer=Project
// 依据文件存在性与内容：
//   Tauri: src-tauri/tauri.conf.json | tauri.conf.json | Cargo.toml 依赖含 tauri
//   Electron: package.json deps/devDeps 含 electron | electron-builder.yml | forge.config.*
//   Node: package.json
//   Rust: Cargo.toml
//   DotNet/WinUi: *.csproj/*.sln；WinUi 额外看 csproj 引用 Microsoft.WindowsAppSDK
//   Python: pyproject.toml | requirements.txt

use std::collections::BTreeMap;
use std::path::Path;

use uenv_core::{Cost, DetectStatus, FactValue, Layer};

use crate::context::ScanContext;
use crate::detector::{Detector, DetectorMeta, DetectorResult};

pub struct ProjectKind;

impl Detector for ProjectKind {
    fn meta(&self) -> DetectorMeta {
        DetectorMeta {
            id: "project.kind",
            layer: Layer::Project,
            title: "项目类型",
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

        let (kinds, markers) = detect_kinds(&root);

        // 不是项目 → Skipped（硬规则：目录不是项目时 Skipped，不是 Error）
        if kinds.is_empty() {
            return DetectorResult {
                status: DetectStatus::Skipped,
                summary: "目录不是已知项目类型".to_string(),
                facts: BTreeMap::new(),
                volatile: BTreeMap::new(),
                evidence: vec![],
            };
        }

        let kinds_set: Vec<FactValue> = kinds
            .iter()
            .map(|k| FactValue::Str(k.to_string()))
            .collect();
        let markers_set: Vec<FactValue> = markers
            .iter()
            .map(|m| FactValue::Str(m.to_string()))
            .collect();

        let mut facts = BTreeMap::new();
        facts.insert("kinds".to_string(), FactValue::Set(kinds_set));
        facts.insert("markers".to_string(), FactValue::Set(markers_set));

        DetectorResult {
            status: DetectStatus::Ok,
            summary: kinds.join("+"),
            facts,
            volatile: BTreeMap::new(),
            evidence: vec![],
        }
    }
}

/// 判定项目类型 —— 与 IO 分离，独立可测。
/// 返回 (kinds, markers)。markers 是命中的文件相对路径（/ 分隔）。
pub fn detect_kinds(root: &Path) -> (Vec<String>, Vec<String>) {
    let mut kinds: Vec<String> = Vec::new();
    let mut markers: Vec<String> = Vec::new();

    let rel = |p: &str| p.to_string();

    // Node: package.json
    let has_package_json = root.join("package.json").is_file();
    if has_package_json {
        kinds.push("node".to_string());
        markers.push(rel("package.json"));
    }

    // Rust: Cargo.toml
    let has_cargo = root.join("Cargo.toml").is_file();
    if has_cargo {
        kinds.push("rust".to_string());
        markers.push(rel("Cargo.toml"));
    }

    // Tauri: tauri.conf.json 或 Cargo.toml 依赖 tauri
    let tauri_conf =
        root.join("src-tauri/tauri.conf.json").is_file() || root.join("tauri.conf.json").is_file();
    if tauri_conf {
        kinds.push("tauri".to_string());
        markers.push(rel("tauri.conf.json"));
    }
    if has_cargo && cargo_depends_on_tauri(&root.join("Cargo.toml")) {
        if !kinds.contains(&"tauri".to_string()) {
            kinds.push("tauri".to_string());
        }
        markers.push(rel("Cargo.toml (tauri dep)"));
    }

    // Electron: package.json deps 含 electron 或 electron-builder.yml / forge.config.*
    if has_package_json && package_json_has_electron(&root.join("package.json")) {
        kinds.push("electron".to_string());
        markers.push(rel("package.json (electron dep)"));
    }
    if root.join("electron-builder.yml").is_file() {
        kinds.push("electron".to_string());
        markers.push(rel("electron-builder.yml"));
    }
    // forge.config.js / .ts / .cjs / .mjs
    for name in [
        "forge.config.js",
        "forge.config.ts",
        "forge.config.cjs",
        "forge.config.mjs",
    ] {
        if root.join(name).is_file() {
            kinds.push("electron".to_string());
            markers.push(rel(name));
            break;
        }
    }

    // DotNet / WinUi: *.csproj / *.sln
    let csproj = find_glob(root, "*.csproj");
    let sln = find_glob(root, "*.sln");
    if !csproj.is_empty() || !sln.is_empty() {
        kinds.push("dotnet".to_string());
        if let Some(f) = csproj.first() {
            markers.push(f.clone());
        } else if let Some(f) = sln.first() {
            markers.push(f.clone());
        }
        // WinUi: csproj 引用 Microsoft.WindowsAppSDK
        if csproj
            .iter()
            .any(|f| csproj_refs_windowsappsdk(Path::new(f)))
        {
            kinds.push("winui".to_string());
            markers.push(format!("{} (WindowsAppSDK)", csproj[0]));
        }
    }

    // Python: pyproject.toml / requirements.txt
    if root.join("pyproject.toml").is_file() {
        kinds.push("python".to_string());
        markers.push(rel("pyproject.toml"));
    }
    if root.join("requirements.txt").is_file() {
        kinds.push("python".to_string());
        markers.push(rel("requirements.txt"));
    }

    (kinds, markers)
}

/// Cargo.toml 依赖段是否含 tauri（粗查：依赖名恰好等于 tauri）
fn cargo_depends_on_tauri(cargo_toml: &Path) -> bool {
    let content = match std::fs::read_to_string(cargo_toml) {
        Ok(c) => c,
        Err(_) => return false,
    };
    content
        .lines()
        .any(|l| l.trim().starts_with("tauri") && !l.trim_start().starts_with('#'))
}

/// package.json deps/devDeps 是否含 electron（粗查：JSON 里有 "electron": 键）
fn package_json_has_electron(package_json: &Path) -> bool {
    let content = match std::fs::read_to_string(package_json) {
        Ok(c) => c,
        Err(_) => return false,
    };
    content.contains("\"electron\"")
}

/// csproj 是否引用 Microsoft.WindowsAppSDK
fn csproj_refs_windowsappsdk(csproj: &Path) -> bool {
    let content = match std::fs::read_to_string(csproj) {
        Ok(c) => c,
        Err(_) => return false,
    };
    content.contains("Microsoft.WindowsAppSDK")
}

/// 顶层目录 glob（只匹配非递归文件名，不引入 glob crate）
fn find_glob(root: &Path, pattern: &str) -> Vec<String> {
    let ext = pattern.trim_start_matches('*');
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with(ext) && e.path().is_file() {
                out.push(name);
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_rust_only() {
        let dir = std::env::temp_dir().join(format!("uenv-kind-rust-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
        let (kinds, _) = detect_kinds(&dir);
        assert!(kinds.contains(&"rust".to_string()));
        assert!(!kinds.contains(&"node".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn kind_tauri_via_conf() {
        let dir = std::env::temp_dir().join(format!("uenv-kind-tauri-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src-tauri")).unwrap();
        std::fs::write(dir.join("package.json"), "{}").unwrap();
        std::fs::write(dir.join("src-tauri/tauri.conf.json"), "{}").unwrap();
        let (kinds, markers) = detect_kinds(&dir);
        assert!(kinds.contains(&"tauri".to_string()));
        assert!(kinds.contains(&"node".to_string()));
        assert!(markers.iter().any(|m| m == "tauri.conf.json"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn kind_electron_via_dep() {
        let dir = std::env::temp_dir().join(format!("uenv-kind-elec-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            r#"{"devDependencies":{"electron":"^28.0.0"}}"#,
        )
        .unwrap();
        let (kinds, _) = detect_kinds(&dir);
        assert!(kinds.contains(&"electron".to_string()));
        assert!(kinds.contains(&"node".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn kind_python() {
        let dir = std::env::temp_dir().join(format!("uenv-kind-py-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pyproject.toml"), "").unwrap();
        let (kinds, _) = detect_kinds(&dir);
        assert!(kinds.contains(&"python".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn kind_not_a_project() {
        let dir = std::env::temp_dir().join(format!("uenv-kind-none-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (kinds, _) = detect_kinds(&dir);
        assert!(kinds.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
