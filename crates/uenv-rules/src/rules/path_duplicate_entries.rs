// path.duplicate-entries — PATH 有重复条目。
// Info / confirm

#[allow(unused_imports)]
use uenv_core::{Environment, Finding, Safety, Severity};

use crate::Rule;
use crate::helpers::{FindingExt, fact_collection_len, finding};

pub struct PathDuplicateEntries;

impl Rule for PathDuplicateEntries {
    fn id(&self) -> &'static str {
        "path.duplicate-entries"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["path.analysis"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        let Some(n) = fact_collection_len(env, "path.analysis", "duplicates") else {
            return vec![];
        };
        if n == 0 {
            return vec![];
        }
        vec![finding(
            self.id(),
            Severity::Info,
            "PATH 里有重复条目",
            &format!(
                "同一个目录在 PATH 里出现了 {n} 组重复。重复本身不致命，但会让排查「你机器上到底哪个 exe 生效」变得更难——which 命中的顺序完全取决于 PATH 顺序，重复条目会掩盖 nvm/volta/fnm 之类的版本切换问题。建议顺手清理，尤其是安装多个工具链后残留的旧路径。"
            ),
        )
        .with_fix(
            Safety::Confirm,
            "清理 PATH 重复条目（用户级，去重后写回）",
            &[
                "powershell -NoProfile -Command \"$p=[Environment]::GetEnvironmentVariable('Path','User'); [Environment]::SetEnvironmentVariable('Path',(($p -split ';' | Select-Object -Unique) -join ';'),'User')\"",
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
    fn triggers_on_duplicates() {
        let mut env = crate::test_utils::empty_env();
        crate::test_utils::with_detector(
            &mut env,
            "path.analysis",
            Layer::Toolchain,
            BTreeMap::from([(
                "duplicates".to_string(),
                FactValue::Set(vec![FactValue::Str("c:\\windows".to_string())]),
            )]),
        );
        assert_eq!(PathDuplicateEntries.evaluate(&env).len(), 1);
        // fix 存在且 commands/rollback 非空
        let f = &PathDuplicateEntries.evaluate(&env)[0];
        let fix = f.suggested_fix.as_ref().unwrap();
        assert!(!fix.commands.is_empty());
        assert!(!fix.rollback.is_empty());
    }

    #[test]
    fn silent_without_duplicates() {
        let mut env = crate::test_utils::empty_env();
        crate::test_utils::with_detector(
            &mut env,
            "path.analysis",
            Layer::Toolchain,
            BTreeMap::new(),
        );
        assert_eq!(PathDuplicateEntries.evaluate(&env).len(), 0);
        assert_eq!(
            PathDuplicateEntries
                .evaluate(&crate::test_utils::empty_env())
                .len(),
            0
        );
    }
}
