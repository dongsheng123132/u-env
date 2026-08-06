// helpers — 从 Environment 读 detector facts 的便捷工具。
// 规则只读 Environment，这里统一封装"取 fact"的路径。

use std::collections::BTreeMap;

use uenv_core::{DetectStatus, Environment, FactValue, Finding, Safety, Severity, SuggestedFix};

/// 取某 detector 的 facts（BTreeMap），detector 不存在/Error → 空 map
pub fn detector_facts<'a>(env: &'a Environment, id: &str) -> BTreeMap<&'a str, &'a FactValue> {
    let mut out = BTreeMap::new();
    if let Some(record) = env.detectors.get(id) {
        if record.status != DetectStatus::Error {
            for (k, v) in &record.facts {
                out.insert(k.as_str(), v);
            }
        }
    }
    out
}

/// 取 detector 的某个 fact，detector 缺失或 status=Error 时返回 None
pub fn fact<'a>(env: &'a Environment, detector: &str, key: &str) -> Option<&'a FactValue> {
    let record = env.detectors.get(detector)?;
    if record.status == DetectStatus::Error {
        return None;
    }
    record.facts.get(key)
}

/// 取 Str fact 的值
pub fn fact_str(env: &Environment, detector: &str, key: &str) -> Option<String> {
    match fact(env, detector, key)? {
        FactValue::Str(s) => Some(s.clone()),
        _ => None,
    }
}

/// 取 Bool fact 的值
pub fn fact_bool(env: &Environment, detector: &str, key: &str) -> Option<bool> {
    match fact(env, detector, key)? {
        FactValue::Bool(b) => Some(*b),
        _ => None,
    }
}

/// 取 Set/List 的元素数
pub fn fact_collection_len(env: &Environment, detector: &str, key: &str) -> Option<usize> {
    match fact(env, detector, key)? {
        FactValue::Set(s) => Some(s.len()),
        FactValue::List(l) => Some(l.len()),
        _ => None,
    }
}

/// Set 里是否包含指定字符串
pub fn fact_set_contains(env: &Environment, detector: &str, key: &str, needle: &str) -> bool {
    match fact(env, detector, key) {
        Some(FactValue::Set(items)) => items.iter().any(|i| match i {
            FactValue::Str(s) => s == needle,
            _ => false,
        }),
        _ => false,
    }
}

/// 项目 kinds 集合（project.kind 的 facts.kinds）
pub fn project_kinds(env: &Environment) -> Vec<String> {
    match fact(env, "project.kind", "kinds") {
        Some(FactValue::Set(items)) => items
            .iter()
            .filter_map(|i| match i {
                FactValue::Str(s) => Some(s.clone()),
                _ => None,
            })
            .collect(),
        _ => vec![],
    }
}

/// 项目是否属于某 kind
pub fn kind_is(env: &Environment, kind: &str) -> bool {
    project_kinds(env).iter().any(|k| k == kind)
}

/// 从 project.drift 取某工具的 satisfied（"true"/"false"/"unknown"）
pub fn drift_satisfied(env: &Environment, tool: &str) -> Option<String> {
    let drift = fact(env, "project.drift", "drift")?;
    let FactValue::Map(tools) = drift else {
        return None;
    };
    let tool_map = tools.get(tool)?;
    let FactValue::Map(m) = tool_map else {
        return None;
    };
    match m.get("satisfied")? {
        FactValue::Bool(true) => Some("true".to_string()),
        FactValue::Bool(false) => Some("false".to_string()),
        FactValue::Str(s) => Some(s.clone()),
        _ => None,
    }
}

/// drift 判定明确为 false（声明不满足）
pub fn drift_satisfies_false(env: &Environment, tool: &str) -> bool {
    drift_satisfied(env, tool).as_deref() == Some("false")
}

/// 项目是否在 project.drift 里声明了某工具
pub fn drift_declares(env: &Environment, tool: &str) -> bool {
    let Some(drift) = fact(env, "project.drift", "drift") else {
        return false;
    };
    matches!(drift, FactValue::Map(m) if m.contains_key(tool))
}

/// 从 project.lockfiles 取 lockfile 文件名列表
pub fn lockfile_names(env: &Environment) -> Vec<String> {
    match fact(env, "project.lockfiles", "lockfiles") {
        Some(FactValue::Map(m)) => m.keys().cloned().collect(),
        _ => vec![],
    }
}

/// 构造一条 Finding 的快捷方式
pub fn finding(
    rule_id: &str,
    severity: Severity,
    title: &str,
    description: &str,
) -> Finding {
    Finding {
        rule_id: rule_id.to_string(),
        severity,
        title: title.to_string(),
        description: description.to_string(),
        evidence: vec![],
        suggested_fix: None,
    }
}

/// Finding 扩展：挂 SuggestedFix（commands 非空则 rollback 必须非空）
pub trait FindingExt {
    fn with_fix(
        self,
        safety: Safety,
        explain: &str,
        commands: &[&str],
        rollback: &[&str],
    ) -> Self;
}

impl FindingExt for Finding {
    fn with_fix(
        mut self,
        safety: Safety,
        explain: &str,
        commands: &[&str],
        rollback: &[&str],
    ) -> Self {
        self.suggested_fix = Some(SuggestedFix {
            safety,
            explain: explain.to_string(),
            commands: commands.iter().map(|c| c.to_string()).collect(),
            rollback: rollback.iter().map(|c| c.to_string()).collect(),
            docs_url: None,
        });
        self
    }
}
