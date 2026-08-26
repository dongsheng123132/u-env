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
        .with_executable_fix(
            Safety::Confirm,
            "移除用户级 PATH 里不存在的目录（执行前先把原值备份到 %USERPROFILE%\\.uenv-path-backup.txt，撤销时从这份快照精确还原）",
            // 真事务：第 1 步落执行时快照，第 2 步才写新值。
            // rollback 只认备份文件里的原串，且仅当现 PATH 仍等于本次写入值时
            // 才还原；中途被别的程序改过就报冲突退出，绝不覆盖。
            &[
                "powershell -NoProfile -Command \"$b=Join-Path $env:USERPROFILE '.uenv-path-backup.txt'; $p=[Environment]::GetEnvironmentVariable('Path','User'); Set-Content -Path $b -Value $p -NoNewline -Encoding UTF8\"",
                "powershell -NoProfile -Command \"$p=[Environment]::GetEnvironmentVariable('Path','User'); $keep=($p -split ';' | Where-Object { $_ -and (Test-Path $_) }) -join ';'; if($keep -ne $p){[Environment]::SetEnvironmentVariable('Path',$keep,'User'); Write-Host '已移除失效条目；原值已存 %USERPROFILE%\\\\.uenv-path-backup.txt'}else{Write-Host 'PATH 无变化'}\"",
            ],
            &[
                "powershell -NoProfile -Command \"$b=Join-Path $env:USERPROFILE '.uenv-path-backup.txt'; if(!(Test-Path $b)){throw '未找到备份文件，拒绝盲恢复'}; $bak=[IO.File]::ReadAllText($b); $cur=[Environment]::GetEnvironmentVariable('Path','User'); $k=(($cur -split ';' | Where-Object { $_ -and (Test-Path $_) }) -join ';'); if($cur -ne $k){throw '当前 PATH 与本次清理结果不一致：中途被其他程序改过，为避免覆盖他人改动而中止。请人工比对 %USERPROFILE%\\\\.uenv-path-backup.txt'}; [Environment]::SetEnvironmentVariable('Path',$bak,'User'); Write-Host '已从执行前快照还原'",
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
