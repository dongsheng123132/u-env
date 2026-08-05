use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::types::ProjectKind;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectManifest {
    pub root: String,
    pub kind: Vec<ProjectKind>,
    pub declared_toolchains: BTreeMap<String, String>,
    pub lockfiles: BTreeMap<String, String>,
    pub git: Option<GitState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitState {
    pub branch: String,
    pub commit: String,
    pub dirty: bool,
}
