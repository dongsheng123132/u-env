use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::types::{Architecture, DetectStatus, Evidence, FactValue, Layer};

/// scan 的顶层产物 = environment.origin.json
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Environment {
    pub spec: String,
    /// RFC3339，不进指纹
    pub generated_at: String,
    /// CLI 版本，不进指纹
    pub uenv_version: String,
    pub identity: EnvironmentIdentity,
    pub detectors: BTreeMap<String, DetectorRecord>,
    pub fingerprint: Option<crate::fingerprint::EnvironmentFingerprint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnvironmentIdentity {
    /// 脱敏后的机器标识，默认 "<host>"
    pub host_alias: String,
    pub os: OperatingSystem,
    pub architecture: Architecture,
    pub project: Option<crate::project::ProjectManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OperatingSystem {
    pub family: String,
    pub product_name: String,
    pub version: String,
    pub build: u32,
    pub ubr: Option<u32>,
    pub edition: Option<String>,
    pub display_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetectorRecord {
    pub id: String,
    pub layer: Layer,
    pub title: String,
    pub status: DetectStatus,
    pub summary: String,
    /// 进指纹
    pub facts: BTreeMap<String, FactValue>,
    /// 不进指纹
    pub volatile: BTreeMap<String, FactValue>,
    /// 不进指纹
    pub evidence: Vec<Evidence>,
    /// 不进指纹
    pub elapsed_ms: u64,
}
