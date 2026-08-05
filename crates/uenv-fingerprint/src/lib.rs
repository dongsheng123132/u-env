// uenv-fingerprint — 指纹计算 + diff。T0 仅建 crate，完整实现在 T4。

/// 占位：待 T4 实现。
pub fn compute_fingerprint(
    _env: &uenv_core::Environment,
) -> anyhow::Result<uenv_core::EnvironmentFingerprint> {
    todo!("fingerprint computing — T4")
}

/// 占位：待 T4 实现。
pub fn diff(_a: &uenv_core::Environment, _b: &uenv_core::Environment) -> anyhow::Result<String> {
    todo!("environment diff — T4")
}
