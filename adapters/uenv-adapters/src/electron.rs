// electron adapter — Electron 框架的 capability 与相关 detector 声明。

use super::{Adapter, AdapterMeta, env_has_kind};
use uenv_core::Environment;

pub struct ElectronAdapter;

impl Adapter for ElectronAdapter {
    fn meta(&self) -> AdapterMeta {
        AdapterMeta {
            id: "electron",
            required_capabilities: &["desktop-ui", "embedded-web-renderer", "native-toolchain"],
            relevant_detectors: &[
                "toolchain.npm-family",
                "toolchain.node",
                "toolchain.msvc",
                "net.proxy",
                "security.defender",
            ],
        }
    }

    fn matches(&self, env: &Environment) -> bool {
        env_has_kind(env, "electron")
    }
}
