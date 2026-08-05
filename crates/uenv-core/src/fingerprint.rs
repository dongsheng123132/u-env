use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnvironmentFingerprint {
    /// origin-env:sha256:<64hex>
    pub host: String,
    pub toolchain: String,
    pub project: Option<String>,
    pub full: String,
}
