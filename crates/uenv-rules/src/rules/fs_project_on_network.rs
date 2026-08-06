// fs.project-on-network — 规则。
// severity: Severity::Error

#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use uenv_core::{DetectStatus, Environment, FactValue, Finding, Layer, Safety, Severity};

use crate::Rule;
use crate::helpers::{FindingExt, finding};

pub struct FsProjectOnNetwork;

impl Rule for FsProjectOnNetwork {
    fn id(&self) -> &'static str {
        "fs.project-on-network"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["fs.project-location"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        let desc = "项目放在网络盘（UNC 或映射盘）。编译器对网络延迟极其敏感——Rust 增量编译、npm install、git 操作都会慢一个数量级，且网络抖动会直接造成构建失败或仓库损坏。开发项目必须放本地 SSD。";
        let fix_safety = Safety::Manual;
        let fix_explain = "把项目复制到本地盘开发，网络盘只做备份/分发";
        let fix_commands: &[&str] = &["echo \"复制到本地：robocopy Z:\\proj C:\\dev\\proj /E\""];
        let fix_rollback: &[&str] = &["（无法自动回滚——涉及项目迁移）"];
        let on = crate::helpers::fact_bool(env, "fs.project-location", "on_network");
        let Some(true) = on else {
            return vec![];
        };
        vec![
            finding(self.id(), Severity::Error, "项目在网络盘上", desc).with_fix(
                fix_safety,
                fix_explain,
                fix_commands,
                fix_rollback,
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_triggers_and_silent() {
        let mut env = crate::test_utils::empty_env();
        crate::test_utils::with_detector(
            &mut env,
            "fs.project-location",
            Layer::Host,
            BTreeMap::from([("on_network".to_string(), crate::test_utils::b(true))]),
        );
        assert_eq!(FsProjectOnNetwork.evaluate(&env).len(), 1);
        assert_eq!(
            FsProjectOnNetwork
                .evaluate(&crate::test_utils::empty_env())
                .len(),
            0
        );
    }
}
