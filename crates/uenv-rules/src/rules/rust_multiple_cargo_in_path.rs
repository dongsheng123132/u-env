// rust.multiple-cargo-in-path — PATH 里命中多个 cargo。
// Warning / confirm

#[allow(unused_imports)]
use uenv_core::{Environment, Finding, Safety, Severity};

use crate::helpers::{fact_collection_len, finding, FindingExt};
use crate::Rule;

pub struct RustMultipleCargoInPath;

impl Rule for RustMultipleCargoInPath {
    fn id(&self) -> &'static str {
        "rust.multiple-cargo-in-path"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["toolchain.rust", "path.analysis"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        let Some(n) = fact_collection_len(env, "toolchain.rust", "cargo_paths") else {
            return vec![];
        };
        if n <= 1 {
            return vec![];
        }
        vec![finding(
            self.id(),
            Severity::Warning,
            "PATH 里有多个 cargo",
            &format!(
                "PATH 命中了 {n} 个 cargo.exe。这比多 Node 更阴险：cargo 会按它自己编译时记录的路径调用 rustc，两个 cargo 通常各带一个 rustc，版本不一致时 `cargo build` 和 `rustc --version` 说的根本不是一回事，增量编译缓存（target/）也会互相污染。rustup 管理的唯一正确姿势是只用 `~/.cargo/bin` 里的 cargo，其它全删。"
            ),
        )
        .with_fix(
            Safety::Confirm,
            "列出 PATH 中全部 cargo 位置，确认后保留 rustup 的 ~/.cargo/bin 一份",
            &[
                "where cargo",
                "rustup show active-toolchain",
            ],
            &[
                "（无自动回滚——只读命令，改动需手动在系统设置里撤）",
            ],
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use uenv_core::{FactValue, Layer};

    fn env_with_cargo_count(n: usize) -> uenv_core::Environment {
        let mut env = crate::test_utils::empty_env();
        let paths: Vec<FactValue> = (0..n)
            .map(|i| FactValue::Path(format!("C:\\cargo-{i}\\cargo.exe")))
            .collect();
        crate::test_utils::with_detector(&mut env, "toolchain.rust", Layer::Toolchain,
            BTreeMap::from([("cargo_paths".to_string(), FactValue::Set(paths))]));
        env
    }

    #[test]
    fn triggers_on_multiple() {
        let out = RustMultipleCargoInPath.evaluate(&env_with_cargo_count(2));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Warning);
    }

    #[test]
    fn silent_single() {
        assert_eq!(RustMultipleCargoInPath.evaluate(&env_with_cargo_count(1)).len(), 0);
    }
}
