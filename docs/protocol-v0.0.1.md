# 本境协议草案 v0.0.1（Origin Environment Protocol）

> **本草案由 T0-T5 的实现归纳而来，随实现演进。**
> 只写真实跑出来的东西，不发明还没实现的抽象。

## 0. 一句话

代码定义软件做什么，**本境定义软件在哪个世界里必然能工作**。
本协议把「环境」定义为可扫描、可比对、可重现的对象。

## 1. 核心对象

### Environment（环境对象）

一次扫描的完整产物，文件后缀 `.origin.json`，`spec` 字段标记协议版本：

```json
{
  "spec": "origin-environment/v0.1",
  "generated_at": "2026-08-06T00:00:00Z",
  "uenv_version": "0.0.1",
  "identity": { "host_alias": "<host>", "os": { ... }, "architecture": "x64", "project": null },
  "detectors": { "<detector_id>": { ... } },
  "fingerprint": null
}
```

JSON Schema：`schemas/environment.schema.json`（由 uenv-core 导出，勿手写）。

### Detector（检测器）

把环境的一个侧面变成**事实**的单元。三层：`Host`（宿主机）/ `Toolchain`（工具链）/ `Project`（项目）。

| 层 | 数量 | 例子 |
|---|---|---|
| Host | 10 | windows.version · windows.long-paths · wsl.status · net.proxy · host.hardware |
| Toolchain | 9 | toolchain.node · toolchain.rust · toolchain.msvc · runtime.webview2 |
| Project | 5 | project.kind · project.manifests · project.lockfiles · project.git · project.drift |

**核心不变式：scan 只产出事实，不做判断。**「这是个问题」的判断只发生在规则层。

### FactValue（事实值）

有限值类型，带类型标记：`str` / `int` / `bool` / `version` / `path` / `list` / `set` / `map`。
`version` 与 `str` 语义不同（`1.88` ≠ `1.88.0`），`set` 无序、`list` 有序。

### Finding（诊断）与 SuggestedFix（修复建议）

```
Finding { rule_id, severity(info|warning|error), title, description, evidence, suggested_fix }
SuggestedFix { safety(safe|confirm|manual), explain, commands[], rollback[], docs_url }
```

**不变量：commands 非空则 rollback 必非空。**

### Fingerprint（环境指纹）

`origin-env:sha256:<64hex>`。同一台机器的有效环境两次扫描 hash 必须相同。

```
host_input      = { detector_id: facts }  仅 layer == Host
toolchain_input = { detector_id: facts }  仅 layer == Toolchain
project_input   = { detector_id: facts }  仅 layer == Project（无项目则 None）
full_input      = { "host": ..., "toolchain": ..., "project": ... }
<x> = "origin-env:sha256:" + sha256(canonical_json(<x>_input))
```

- 只收 `facts`；`volatile` / `evidence` / `elapsed_ms` / `generated_at` 永不进指纹
- `status == Error` 的 detector 不参与指纹，但必须在结果里列出 `excluded_detectors`
- 规范化规则（§5.1）：Str trim+空白折叠；Path 反斜杠→正斜杠+盘符大写；Set 排序去重；Map 删空值
- diff 的风险分级来自规则的显式关键键清单，不自动猜

## 2. 工作流

```
scan ──► environment.origin.json ──► fingerprint（状态哈希）
  │                                   diff（好机器 vs 坏机器）
  └─► doctor（规则 → Findings）──► report（人读/机器读）
```

## 3. 已实现的工具链（T0-T5 实测）

- 26 个 detector（Host 10 + Toolchain 9 + Project 5 + 样板 2）
- 24 条规则（PATH/Node/Rust/MSVC/WebView2/网络/文件系统/Defender/Python）
- 指纹：确定性已验证（两次扫描 hash 逐字相同）
- diff：`uenv diff a.json b.json` 三段输出（高风险/低风险/仅一侧存在）
- 3 个框架适配器（Tauri/Electron/Node）

## 4. 占位（尚未实现，勿视为协议内容）

- **Capsule（故障舱）**：把出错现场脱敏后固化为可重现对象 —— 规划中
- **Recipe（环境模板）**：参考环境快照 —— 规划中
- 修复执行（`suggested_fix.commands` 的自动执行）—— 规划中

## 5. 与相邻协议的关系

| 协议 | 回答的问题 |
|---|---|
| 本象协议（Origin IR） | 这个对象本来是什么 |
| 影核协议（ActionParity） | 这个对象能够做什么 |
| **本境协议（本文件）** | 这个对象在什么条件下能够正确存在和运行 |
