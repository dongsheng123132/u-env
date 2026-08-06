//! Agent 发现 stub —— 给 AI 指路用，只读安全。
//!
//! 灵感来自 WrenAI 的 Discovery stub（`npx skills add Canner/WrenAI`）：
//! 引导文件极小、只指路，真值按需取、随版本走。
//! 本 stub 由 `uenv stub` 输出/写入，版本号在生成时钉住（CARGO_PKG_VERSION）。

/// 生成 agent 发现 stub 文本。版本在编译期钉住，杜绝「文档先于实现飘走」。
pub fn stub_text() -> String {
    format!(
        r#"# uenv — 环境检查 stub（由 uenv v{version} 生成）

> 给 AI agent 读。你只需要知道一件事：判断这台机器 / 这个项目能不能构建、能不能跑。

## 何时用
要判断本机/本项目是否能构建、是否能跑，或环境哪里不对劲时。

## 怎么用（只读、安全、不修改任何东西）
    uenv doctor --project . --agent

stdout 是结构化 JSON（severity / title / evidence / suggested_fix）。找不到 uenv 时先 `where uenv`。

## 需要更多
    uenv report              # 人类可读报告（markdown）
    uenv scan --out env.json # 环境快照
    uenv --help              # 全部命令

## 版本钉住
本 stub 由 uenv v{version} 生成。若本机 uenv 版本不同，重新生成：
    uenv stub --out <此文件路径>
"#,
        version = env!("CARGO_PKG_VERSION"),
    )
}

#[cfg(test)]
mod tests {
    use super::stub_text;

    #[test]
    fn stub_points_at_doctor_agent() {
        let s = stub_text();
        assert!(s.contains("uenv doctor --project . --agent"));
        assert!(s.contains("只读"));
    }

    #[test]
    fn stub_is_version_pinned() {
        let s = stub_text();
        assert!(s.contains(env!("CARGO_PKG_VERSION")));
        assert!(s.contains("版本钉住"));
    }
}
