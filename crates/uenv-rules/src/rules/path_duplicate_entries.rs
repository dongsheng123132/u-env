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
        .with_executable_fix(
            Safety::Confirm,
            "清理用户级 PATH 重复条目（去重后写回；执行前先把原值备份到 %USERPROFILE%\\.uenv-path-backup.txt，撤销时从这份快照精确还原）",
            // 真事务：第 1 步落执行时快照，第 2 步才写新值。
            // rollback 不重读「当前值」——那会把改完的状态再固化一遍（伪回滚）；
            // 它只认备份文件里的原串，且仅当现 PATH 仍等于本次写入值时才还原，
            // 中途被别的程序改过就报冲突退出，绝不覆盖。
            &[
                "powershell -NoProfile -Command \"$b=Join-Path $env:USERPROFILE '.uenv-path-backup.txt'; $p=[Environment]::GetEnvironmentVariable('Path','User'); Set-Content -Path $b -Value $p -NoNewline -Encoding UTF8\"",
                "powershell -NoProfile -Command \"$p=[Environment]::GetEnvironmentVariable('Path','User'); $d=($p -split ';' | Select-Object -Unique) -join ';'; if($d -ne $p){[Environment]::SetEnvironmentVariable('Path',$d,'User'); Write-Host '已去重；原值已存 %USERPROFILE%\\\\.uenv-path-backup.txt'}else{Write-Host 'PATH 无变化'}\"",
            ],
            &[
                "powershell -NoProfile -Command \"$b=Join-Path $env:USERPROFILE '.uenv-path-backup.txt'; if(!(Test-Path $b)){throw '未找到备份文件，拒绝盲恢复'}; $bak=[IO.File]::ReadAllText($b); $cur=[Environment]::GetEnvironmentVariable('Path','User'); $d=(($cur -split ';' | Select-Object -Unique) -join ';'); if($cur -ne $d){throw '当前 PATH 与本次清理结果不一致：中途被其他程序改过，为避免覆盖他人改动而中止。请人工比对 %USERPROFILE%\\\\.uenv-path-backup.txt'}; [Environment]::SetEnvironmentVariable('Path',$bak,'User'); Write-Host '已从执行前快照还原'",
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
