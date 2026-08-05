// tauri adapter — Tauri 框架的 capability 与相关 detector 声明。

use super::{Adapter, AdapterMeta, env_has_kind};
use uenv_core::Environment;

pub struct TauriAdapter;

impl Adapter for TauriAdapter {
    fn meta(&self) -> AdapterMeta {
        AdapterMeta {
            id: "tauri",
            required_capabilities: &[
                "desktop-ui",
                "embedded-web-renderer",
                "native-toolchain",
                "filesystem-watch",
            ],
            relevant_detectors: &[
                "runtime.webview2",
                "toolchain.rust",
                "toolchain.msvc",
                "toolchain.windows-sdk",
                "toolchain.npm-family",
                "windows.long-paths",
                "fs.project-location",
            ],
        }
    }

    fn matches(&self, env: &Environment) -> bool {
        env_has_kind(env, "tauri")
    }
}
