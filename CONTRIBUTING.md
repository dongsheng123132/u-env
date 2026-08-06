# CONTRIBUTING.md

> **提交一个你遇到过的 Windows Bug，让它以后永远不再困扰其他人。**

贡献被拆得足够小：新贡献者只需完成**其中一种**入口。每种入口都给了
「照着改就能提 PR」的最小例子和文件路径。

## 五个贡献入口

| 入口 | 门槛 | 最小例子 |
|---|---|---|
| **Detector** | 写一个文件，实现一个 trait | 检测 WebView2 版本 / 长路径开关 |
| **Rule** | 写一个文件，实现一个 trait | 两个 Node 共存造成 npm 调用错误 |
| **Adapter** | 支持一个框架 | Flutter Windows / Godot / Unity / Qt |
| **Capsule** | 提交一个脱敏真实故障案例 | 无需写代码 |
| **Recipe** | 提供一个可工作的环境模板 | 无需写代码 |

---

## Detector —— 检测一个新环境事实

一个 detector = 一个文件，抄现有样板即可：

```
crates/uenv-scanner/src/detectors/
├── windows_version.rs   ← 注册表类样板（抄这个）
├── toolchain_node.rs    ← 命令类样板
└── <your_detector>.rs   ← 你的新文件
```

要点（详见 `docs/10-架构与数据模型.md` §4）：

1. 实现 `Detector` trait：`meta()` + `detect(&ScanContext)`，绝 panic、绝返回 Err
2. **解析与 IO 分离**：写 `fn parse(raw: &str) -> BTreeMap<String, FactValue>` 独立可测
3. 每条 fact 必须有 evidence 可追溯
4. 波动值（剩余空间/PID/时间）进 `volatile`，不进 facts

提交时把文件加进 `detectors/mod.rs` 和 `registry.rs::all_detectors()` 各一行。
用 `cargo test -p uenv-scanner` 验证，新 detector 至少 1 个 parse 测试。

## Rule —— 把事实变成诊断

一个规则 = 一个文件，抄现有规则：

```
crates/uenv-rules/src/rules/
├── node_multiple_in_path.rs   ← 样板
└── <your_rule>.rs
```

要点（详见 §6）：

1. 实现 `Rule` trait：`id()` + `relevant_detectors()` + `evaluate(&Environment) -> Vec<Finding>`
2. **规则绝不跑命令、绝不读注册表** —— 跑不出来说明 detector 缺 fact，去加 fact
3. 描述三件事：现象 → 为什么会导致 bug → 怎么办（中文）
4. `SuggestedFix` 的 commands 非空则 rollback 必非空；涉及注册表一律 `confirm`/`manual`

注册进 `crates/uenv-rules/src/lib.rs::all_rules()`，加触发/不触发两个测试。

## Adapter —— 支持一个新框架

一个 adapter = 一个文件：

```
adapters/uenv-adapters/src/
├── tauri.rs      ← 样板
└── <framework>.rs
```

实现 `Adapter` trait：`meta()`（声明 capability + 相关 detector）+ `matches(&Environment)`。
注意依赖方向：adapters → uenv-core（根），不许反向。

## Capsule —— 提交一个真实故障案例（无需写代码）

故障舱是把一个出错现场脱敏后固化的对象。提交方式：

1. 在出问题的机器上跑 `uenv scan --project . --out environment.origin.json`
2. 跑 `uenv doctor --project . --json` 收集诊断
3. 脱敏：确认文件里没有真实用户名/机器名/密钥（`uenv` 默认已脱敏，用 `--no-redact` 跑过就要手工清理）
4. 提交到 issues，标题格式 `[capsule] <一句话现象>`

## Recipe —— 提供一个环境模板（无需写代码）

一个"这机器能跑某类项目"的参考环境快照：`uenv scan` 的输出即可。
放进 `recipes/<name>/environment.origin.json`，供 `uenv diff` 对照。

---

## 开发环境

```bash
cargo fmt --all -- --check        # 0 diff
cargo clippy --workspace --all-targets -- -D warnings   # 0 警告
cargo test --workspace            # 全绿
cargo run -p uenv-cli -- scan --project . --json        # 端到端
```

commit message 用中文，格式 `T<n>: <做了什么>` 或 `<域>: <做了什么>`。
