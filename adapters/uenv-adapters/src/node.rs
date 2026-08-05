// node adapter — Node 项目的 capability 与相关 detector 声明。

use super::{Adapter, AdapterMeta, env_has_kind};
use uenv_core::Environment;

pub struct NodeAdapter;

impl Adapter for NodeAdapter {
    fn meta(&self) -> AdapterMeta {
        AdapterMeta {
            id: "node",
            required_capabilities: &["filesystem-watch"],
            relevant_detectors: &[
                "toolchain.node",
                "toolchain.npm-family",
                "path.analysis",
                "fs.project-location",
                "net.proxy",
            ],
        }
    }

    fn matches(&self, env: &Environment) -> bool {
        env_has_kind(env, "node")
    }
}
