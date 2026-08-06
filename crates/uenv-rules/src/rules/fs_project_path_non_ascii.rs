// fs.project-path-non-ascii — 规则。
// severity: Severity::Warning

#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use uenv_core::{DetectStatus, Environment, FactValue, Finding, Layer, Safety, Severity};

use crate::helpers::{finding, FindingExt};
use crate::Rule;

pub struct FsProjectPathNonAscii;

impl Rule for FsProjectPathNonAscii {
    fn id(&self) -> &'static str {
        "fs.project-path-non-ascii"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["fs.project-location"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        let desc = "项目路径包含中文等非 ASCII 字符。虽然现代工具大多支持 UTF-8 路径，但仍有大量老工具链/原生库按 ANSI 处理路径（尤其 MSVC 老版本、部分 npm 原生模块的编译脚本），会报「无法打开文件」或乱码路径错误。能改则改，至少心里有数。";
        let fix_safety = Safety::Manual;
        let fix_explain = "将项目移到纯 ASCII 路径（如 C:\\dev\\proj）";
        let fix_commands: &[&str] = &["echo \"建议路径只含 a-zA-Z0-9-_\""];
        let fix_rollback: &[&str] = &["（无法自动回滚——涉及项目迁移）"];
        let v = crate::helpers::fact_bool(env, "fs.project-location", "path_has_non_ascii");
        let Some(true) = v else { return vec![]; };
        vec![finding(self.id(), Severity::Warning, "项目路径含非 ASCII 字符", desc).with_fix(fix_safety, fix_explain, fix_commands, fix_rollback)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_triggers_and_silent() {
        let mut env = crate::test_utils::empty_env();
        crate::test_utils::with_detector(&mut env, "fs.project-location", Layer::Host,
            BTreeMap::from([("path_has_non_ascii".to_string(), crate::test_utils::b(true))]));
        assert_eq!(FsProjectPathNonAscii.evaluate(&env).len(), 1);
    }
}
