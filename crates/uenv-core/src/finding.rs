use serde::{Deserialize, Serialize};

use crate::types::{Evidence, Safety, Severity};

/// rules 的产物
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Finding {
    pub rule_id: String,
    pub severity: Severity,
    pub title: String,
    pub description: String,
    pub evidence: Vec<Evidence>,
    pub suggested_fix: Option<SuggestedFix>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SuggestedFix {
    pub safety: Safety,
    pub explain: String,
    pub commands: Vec<String>,
    pub rollback: Vec<String>,
    pub docs_url: Option<String>,
}
