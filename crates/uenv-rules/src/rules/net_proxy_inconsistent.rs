// net.proxy-inconsistent — 规则。
// severity: Severity::Warning

#[allow(unused_imports)]
use std::collections::BTreeMap;
#[allow(unused_imports)]
use uenv_core::{DetectStatus, Environment, FactValue, Finding, Layer, Safety, Severity};

use crate::Rule;
use crate::helpers::{FindingExt, finding};

pub struct NetProxyInconsistent;

impl Rule for NetProxyInconsistent {
    fn id(&self) -> &'static str {
        "net.proxy-inconsistent"
    }

    fn relevant_detectors(&self) -> &'static [&'static str] {
        &["net.proxy"]
    }

    fn evaluate(&self, env: &Environment) -> Vec<Finding> {
        let desc = "系统代理（注册表）和 HTTP_PROXY/HTTPS_PROXY 环境变量指向不同代理，或一边有一边没有。npm/git 走环境变量代理，浏览器走系统代理，两边不一致时「npm 能装但浏览器翻不了墙」或反之，排查网络问题会两头猜。统一到同一个代理地址。";
        let fix_safety = Safety::Manual;
        let fix_explain = "统一代理配置：在系统设置中确认代理地址后，把 HTTP_PROXY/HTTPS_PROXY 环境变量同步成同一个值（涉及环境变量写入与原值记录，暂不自动执行；恢复原环境变量即回滚）";
        let v = crate::helpers::fact_bool(env, "net.proxy", "consistent");
        let Some(false) = v else {
            return vec![];
        };
        vec![
            finding(
                self.id(),
                Severity::Warning,
                "系统代理与环境变量代理不一致",
                desc,
            )
            .with_fix(fix_safety, fix_explain),
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
            "net.proxy",
            Layer::Host,
            BTreeMap::from([("consistent".to_string(), crate::test_utils::b(false))]),
        );
        assert_eq!(NetProxyInconsistent.evaluate(&env).len(), 1);

        let mut env2 = crate::test_utils::empty_env();
        crate::test_utils::with_detector(
            &mut env2,
            "net.proxy",
            Layer::Host,
            BTreeMap::from([("consistent".to_string(), crate::test_utils::b(true))]),
        );
        assert_eq!(NetProxyInconsistent.evaluate(&env2).len(), 0);
    }
}
