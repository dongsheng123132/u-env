// path.missing-entries — PATH 指向不存在的目录。
// Info / confirm

#[allow(unused_imports)]
use uenv_core::{Environment, Finding, Safety, Severity};

use crate::Rule;
use crate::helpers::{FindingExt, fact_collection_len, finding};

pub struct PathMissingEntries;

impl Rule for PathMissingEntries {
    fn id(&self) -> &'static str {
        "path.missing-entries"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["path.analysis"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        let Some(n) = fact_collection_len(env, "path.analysis", "missing") else {
            return vec![];
        };
        if n == 0 {
            return vec![];
        }
        vec![finding(
            self.id(),
            Severity::Info,
            "PATH 里有不存在的目录",
            &format!(
                "{n} 个 PATH 条目指向不存在的目录。这通常是卸载软件后残留的旧路径（比如老版 Node、被删掉的工具目录）。本身不致命——Windows 会静默跳过，但会拖慢每次命令行启动（系统逐个 stat 这些路径），而且等你装回同名工具时行为可能出乎意料。建议定期清理。"
            ),
        )
        .with_fix(
            Safety::Confirm,
            "列出并移除 PATH 里不存在的目录（先预览再确认）",
            &[
                "powershell -NoProfile -Command \"$p=[Environment]::GetEnvironmentVariable('Path','User'); $keep=$p -split ';' | Where-Object { $_ -and (Test-Path $_) }; [Environment]::SetEnvironmentVariable('Path',($keep -join ';'),'User')\"",
            ],
            &[
                "powershell -NoProfile -Command \"$p=[Environment]::GetEnvironmentVariable('Path','User'); [Environment]::SetEnvironmentVariable('Path',$p,'User')\"",
            ],
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use uenv_core::{FactValue, Layer};

    #[test]
    fn triggers_on_missing() {
        let mut env = crate::test_utils::empty_env();
        crate::test_utils::with_detector(
            &mut env,
            "path.analysis",
            Layer::Toolchain,
            BTreeMap::from([(
                "missing".to_string(),
                FactValue::Set(vec![FactValue::Str("D:\\gone".to_string())]),
            )]),
        );
        assert_eq!(PathMissingEntries.evaluate(&env).len(), 1);
    }

    #[test]
    fn silent_ok_path() {
        let mut env = crate::test_utils::empty_env();
        crate::test_utils::with_detector(
            &mut env,
            "path.analysis",
            Layer::Toolchain,
            BTreeMap::from([("missing".to_string(), FactValue::Set(vec![]))]),
        );
        assert_eq!(PathMissingEntries.evaluate(&env).len(), 0);
    }
}
