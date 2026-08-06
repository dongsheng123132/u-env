// rust adapter — Rust 项目的 capability 与相关 detector 声明。
// 与 node/tauri adapter 同级：匹配 project.kind 含 rust 的项目，
// 使 rust.version-drift / rust.gnu-toolchain-on-windows 等规则在 doctor 中生效。

use super::{Adapter, AdapterMeta, env_has_kind};
use uenv_core::Environment;

pub struct RustAdapter;

impl Adapter for RustAdapter {
    fn meta(&self) -> AdapterMeta {
        AdapterMeta {
            id: "rust",
            required_capabilities: &["native-toolchain"],
            relevant_detectors: &[
                "toolchain.rust",
                "toolchain.msvc",
                "toolchain.windows-sdk",
                "windows.long-paths",
                "fs.project-location",
            ],
        }
    }

    fn matches(&self, env: &Environment) -> bool {
        env_has_kind(env, "rust")
    }
}
