use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 有限的值类型 —— 不要用 serde_json::Value，规范化会失控。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FactValue {
    Str(String),
    Int(i64),
    Bool(bool),
    Version(String),
    Path(String),
    List(Vec<FactValue>),
    Set(Vec<FactValue>),
    Map(BTreeMap<String, FactValue>),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    Host,
    Toolchain,
    Project,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DetectStatus {
    Ok,
    Absent,
    Degraded,
    Error,
    Skipped,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Command,
    Registry,
    File,
    Env,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Evidence {
    pub kind: EvidenceKind,
    pub source: String,
    pub exit_code: Option<i32>,
    /// 截断到 2000 字符，已脱敏
    pub excerpt: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Architecture {
    X64,
    Arm64,
    X86,
    Unknown,
}

/// Detector 执行成本分级
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Cost {
    /// < 200ms
    Fast,
    /// >= 200ms
    Slow,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Safety {
    Safe,
    Confirm,
    Manual,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectKind {
    Tauri,
    Electron,
    Node,
    Rust,
    DotNet,
    WinUi,
    Python,
    Unknown,
}
