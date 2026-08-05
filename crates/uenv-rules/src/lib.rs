// uenv-rules — 规则引擎。T0 仅建 crate + 占位 trait，完整实现在 T5。
use uenv_core::{Environment, Finding};

/// 规则只读 Environment，不许跑命令、不许读注册表。
pub trait Rule: Send + Sync {
    fn id(&self) -> &'static str;
    fn evaluate(&self, env: &Environment) -> Vec<Finding>;
}

pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![]
}
