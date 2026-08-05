use std::collections::BTreeMap;

use uenv_core::{Cost, DetectStatus, Evidence, FactValue, Layer};

use crate::context::ScanContext;

#[derive(Debug, Clone)]
pub struct DetectorMeta {
    pub id: &'static str,
    pub layer: Layer,
    pub title: &'static str,
    pub cost: Cost,
}

/// 外部贡献者的唯一入口：实现这个 trait = 一个新的 environment detector。
pub trait Detector: Send + Sync {
    fn meta(&self) -> DetectorMeta;

    /// 不适用时返回 false，会记为 Skipped。默认 true。
    fn applicable(&self, _ctx: &ScanContext) -> bool {
        true
    }

    /// 绝不 panic、绝不返回 Err——任何失败 → status: Error。
    fn detect(&self, ctx: &ScanContext) -> DetectorResult;
}

#[derive(Debug, Clone)]
pub struct DetectorResult {
    pub status: DetectStatus,
    /// 一句话，人读
    pub summary: String,
    pub facts: BTreeMap<String, FactValue>,
    pub volatile: BTreeMap<String, FactValue>,
    pub evidence: Vec<Evidence>,
}
