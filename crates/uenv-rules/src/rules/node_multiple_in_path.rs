// node.multiple-in-path — PATH 里命中多个 node。
// Warning / confirm

#[allow(unused_imports)]
use uenv_core::{Environment, Finding, Safety, Severity};

use crate::helpers::{fact_collection_len, finding, FindingExt};
use crate::Rule;

pub struct NodeMultipleInPath;

impl Rule for NodeMultipleInPath {
    fn id(&self) -> &'static str {
        "node.multiple-in-path"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["toolchain.node", "path.analysis"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        let Some(n) = fact_collection_len(env, "toolchain.node", "executables") else {
            return vec![];
        };
        if n <= 1 {
            return vec![];
        }
        vec![finding(
            self.id(),
            Severity::Warning,
            "PATH 里有多个 Node.js",
            &format!(
                "PATH 命中了 {n} 个 node.exe。你敲 `node` 时到底用哪个，完全取决于 PATH 顺序——今天能用、明天在别的终端里就版本不对，这类「换了个终端就坏」的玄学 bug 十有八九是这个。常见来源：nvm 残留链接 + 全局安装的 Node + %APPDATA%\\npm 路径同时存在。建议统一到一个版本管理器（nvm-windows / fnm / volta）管理。"
            ),
        )
        .with_fix(
            Safety::Confirm,
            "列出 PATH 中全部 node 位置，确认后手动移除多余项",
            &[
                "where node",
                "powershell -NoProfile -Command \"(Get-Command node -All).Source\"",
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

    fn env_with_node_count(n: usize) -> uenv_core::Environment {
        let mut env = crate::test_utils::empty_env();
        let paths: Vec<FactValue> = (0..n)
            .map(|i| FactValue::Path(format!("C:\\node-{i}\\node.exe")))
            .collect();
        crate::test_utils::with_detector(&mut env, "toolchain.node", Layer::Toolchain,
            BTreeMap::from([("executables".to_string(), FactValue::Set(paths))]));
        env
    }

    #[test]
    fn triggers_on_multiple() {
        let out = NodeMultipleInPath.evaluate(&env_with_node_count(2));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].severity, Severity::Warning);
    }

    #[test]
    fn silent_single_node() {
        assert_eq!(NodeMultipleInPath.evaluate(&env_with_node_count(1)).len(), 0);
        assert_eq!(NodeMultipleInPath.evaluate(&crate::test_utils::empty_env()).len(), 0);
    }
}
