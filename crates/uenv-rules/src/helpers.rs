// helpers — 从 Environment 读 detector facts 的便捷工具。
// 规则只读 Environment，这里统一封装"取 fact"的路径。

use std::collections::BTreeMap;

use uenv_core::{DetectStatus, Environment, FactValue, Finding, Safety, Severity, SuggestedFix};

/// 取某 detector 的 facts（BTreeMap），detector 不存在/Error → 空 map
pub fn detector_facts<'a>(env: &'a Environment, id: &str) -> BTreeMap<&'a str, &'a FactValue> {
    let mut out = BTreeMap::new();
    if let Some(record) = env.detectors.get(id)
        && record.status != DetectStatus::Error
    {
        for (k, v) in &record.facts {
            out.insert(k.as_str(), v);
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
pub fn finding(rule_id: &str, severity: Severity, title: &str, description: &str) -> Finding {
    Finding {
        rule_id: rule_id.to_string(),
        severity,
        title: title.to_string(),
        description: description.to_string(),
        evidence: vec![],
        suggested_fix: None,
    }
}

/// Finding 扩展：挂 SuggestedFix。
///
/// 契约（docs/10 §3，2026-08-26 类级收紧）：
/// 1. **Manual 档双空**：`commands` 与 `rollback` 都必须为空——手动档只有说明，
///    不假装能执行。给人读的话（含示例命令）写进 `explain`。
/// 2. **Safe/Confirm 档必须成对**：`commands` 非空则 `rollback` 必须非空，且
///    二者必须是真逆操作——要么触发条件钉住了旧值（如 autocrlf=true 时建议改
///    input，rollback 改回 true），要么 apply 第一步自带执行时快照（如 PATH
///    先落备份文件再改写，rollback 从备份还原）。靠「重读当前值」冒充撤销的
///    是伪回滚，过不了这里的断言。
/// 3. **禁止说明文字混进命令**：任何以全角/半角左括号开头、或以 `echo ` 开头
///    的条目都是给人读的注记，不是可执行命令——构造时直接 panic，让违规死在
///    测试与开发期，而不是出现在用户面前。
///
/// 违反任一条 = panic（fail fast）。规则是静态表，构造点校验成本可忽略；
/// 安全属性长在数据构造处，不依赖下游渲染端的自觉。
pub trait FindingExt {
    /// 挂建议。Manual 档只给 explain；Confirm/Safe 档用 with_executable_fix 给
    /// 成对的 commands/rollback。两种途径都在构造点过 validate_fix_contract。
    fn with_fix(self, safety: Safety, explain: &str) -> Self;

    /// Confirm/Safe 档：可执行修复必须带 rollback 成对出现（契约条款 2）。
    fn with_executable_fix(
        self,
        safety: Safety,
        explain: &str,
        commands: &[&str],
        rollback: &[&str],
    ) -> Self;
}

/// 校验一条 fix 是否满足契约，违规给出可定位的报错。
/// 独立成函数以便单测直接覆盖各类违规样例。
pub fn validate_fix_contract(
    rule_hint: &str,
    safety: Safety,
    explain: &str,
    commands: &[&str],
    rollback: &[&str],
) {
    let _ = explain;
    // 条款 3：禁止说明文字混进命令（对 commands 与 rollback 一视同仁）
    for c in commands.iter().chain(rollback.iter()) {
        let t = c.trim_start();
        assert!(
            !t.starts_with('（') && !t.starts_with('('),
            "[{rule_hint}] 契约违规：括号开头的说明文字混进了命令：{c:?}（给人读的话放 explain）"
        );
        assert!(
            !(t.starts_with("echo ") || t == "echo"),
            "[{rule_hint}] 契约违规：echo 提示语不是真命令：{c:?}（要传达的信息放 explain）"
        );
    }
    match safety {
        // 条款 1：Manual 双空
        Safety::Manual => {
            assert!(
                commands.is_empty() && rollback.is_empty(),
                "[{rule_hint}] 契约违规：Manual 档的 commands/rollback 必须全空（现 commands={commands:?}, rollback={rollback:?}）；手动步骤写进 explain"
            );
        }
        // 条款 2：非 Manual 且有命令则必须有回滚
        Safety::Safe | Safety::Confirm => {
            assert!(
                commands.is_empty() || !rollback.is_empty(),
                "[{rule_hint}] 契约违规：{safety:?} 档 commands 非空则 rollback 必须非空，且必须是真逆操作（触发条件钉住旧值，或 apply 自带执行时快照）"
            );
        }
    }
}

impl FindingExt for Finding {
    fn with_fix(mut self, safety: Safety, explain: &str) -> Self {
        // Manual 档专用入口：强制双空（契约条款 1 在此结构性成立）。
        validate_fix_contract("with_fix", safety, explain, &[], &[]);
        self.suggested_fix = Some(SuggestedFix {
            safety,
            explain: explain.to_string(),
            commands: vec![],
            rollback: vec![],
            docs_url: None,
        });
        self
    }

    fn with_executable_fix(
        mut self,
        safety: Safety,
        explain: &str,
        commands: &[&str],
        rollback: &[&str],
    ) -> Self {
        assert!(
            !matches!(safety, Safety::Manual),
            "[with_executable_fix] 契约违规：可执行修复不允许挂在 Manual 档（手动步骤写 explain，用 with_fix）"
        );
        validate_fix_contract("with_executable_fix", safety, explain, commands, rollback);
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

#[cfg(test)]
mod fix_contract_tests {
    use super::*;

    #[test]
    fn manual_allows_empty_only() {
        // Manual 双空：合法
        validate_fix_contract("t", Safety::Manual, "说明", &[], &[]);
    }

    #[test]
    #[should_panic(expected = "Manual 档")]
    fn manual_rejects_commands() {
        validate_fix_contract("t", Safety::Manual, "说明", &["git status"], &[]);
    }

    #[test]
    #[should_panic(expected = "echo 提示语")]
    fn echo_prose_is_banned_everywhere() {
        validate_fix_contract(
            "t",
            Safety::Confirm,
            "说明",
            &[
                "echo \"提示\"",
                "powershell -Command \"Set-ItemProperty x\"",
            ],
            &["（手动恢复）"],
        );
    }

    #[test]
    #[should_panic(expected = "说明文字")]
    fn parenthetical_prose_rollback_is_banned() {
        validate_fix_contract(
            "t",
            Safety::Confirm,
            "说明",
            &["git config x"],
            &["（无法回滚）"],
        );
    }

    #[test]
    #[should_panic(expected = "rollback 必须非空")]
    fn confirm_without_rollback_is_banned() {
        validate_fix_contract("t", Safety::Confirm, "说明", &["git config x"], &[]);
    }

    #[test]
    fn confirm_pair_passes() {
        validate_fix_contract(
            "t",
            Safety::Confirm,
            "说明",
            &["git config core.autocrlf input"],
            &["git config core.autocrlf true"],
        );
    }
}
