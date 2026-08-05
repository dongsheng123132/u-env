// uenv-fingerprint — 指纹计算 + 环境差异（T4 实现）。

pub mod diff;
pub mod fingerprint;
pub mod normalize;

pub use diff::{FactDiff, Risk, diff_environments, render_json, render_text};
pub use fingerprint::{compute_fingerprint, short};
