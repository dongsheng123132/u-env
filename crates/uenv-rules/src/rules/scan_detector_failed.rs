// scan.detector-failed — 规则。
// severity: Severity::Warning

#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use uenv_core::{DetectStatus, Environment, FactValue, Finding, Layer, Safety, Severity};

use crate::Rule;
use crate::helpers::{FindingExt, finding};

pub struct ScanDetectorFailed;

impl Rule for ScanDetectorFailed {
    fn id(&self) -> &'static str {
        "scan.detector-failed"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &[]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        let desc = "本次扫描有 detector 返回 Error（比如 PowerShell 不可用、权限不足、命令超时）。这些检测器的结论缺失，意味着上面的诊断可能有漏网之鱼——先解决扫描失败项，再信任其余诊断。常见原因：非管理员运行、代理未开、杀软拦截。";
        let fix_safety = Safety::Manual;
        let fix_explain = "以管理员身份重跑 uenv doctor，或检查失败的检测器对应的服务";
        let fix_commands: &[&str] = &["echo \"以管理员身份重跑：uenv doctor --project .\""];
        let fix_rollback: &[&str] = &["（无回滚）"];
        let failed: Vec<&str> = env
            .detectors
            .iter()
            .filter(|(_, r)| r.status == DetectStatus::Error)
            .map(|(id, _)| id.as_str())
            .collect();
        if failed.is_empty() {
            return vec![];
        }
        let list = failed.join(", ");
        vec![
            finding(
                self.id(),
                Severity::Warning,
                "部分环境检测失败",
                &format!("{desc}（失败项：{list}）"),
            )
            .with_fix(fix_safety, fix_explain, fix_commands, fix_rollback),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_triggers_and_silent() {
        let mut env = crate::test_utils::empty_env();
        crate::test_utils::with_error_detector(&mut env, "net.proxy");
        assert_eq!(ScanDetectorFailed.evaluate(&env).len(), 1);
        assert_eq!(
            ScanDetectorFailed
                .evaluate(&crate::test_utils::empty_env())
                .len(),
            0
        );
    }
}
