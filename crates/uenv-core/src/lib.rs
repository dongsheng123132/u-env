// uenv-core — u-env 数据模型，零 Windows 调用，零其他 uenv 依赖。
// 规格来源：docs/10-架构与数据模型.md §3

mod environment;
mod finding;
mod fingerprint;
mod project;
mod types;

pub use environment::{DetectorRecord, Environment, EnvironmentIdentity, OperatingSystem};
pub use finding::{Finding, SuggestedFix};
pub use fingerprint::EnvironmentFingerprint;
pub use project::{GitState, ProjectManifest};
pub use types::{
    Architecture, Cost, DetectStatus, Evidence, EvidenceKind, FactValue, Layer, ProjectKind,
    Safety, Severity,
};
