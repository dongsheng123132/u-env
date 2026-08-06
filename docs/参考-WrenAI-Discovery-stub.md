# 参考 · WrenAI 的 Discovery stub → uenv 的 agent 上手指南

> 状态：**参考笔记 + MVP 已落地**。2026-08-07 `uenv stub` 子命令已实现（版本钉住、`--out` 写文件、
> JSON 模式），见 [第三节](#三-可落地的形态设想) 的 ✓ 标记。仍未做的是客户端自动检测与多类别按需拉取。
> 出处：[Canner/WrenAI](https://github.com/Canner/WrenAI)（GenBI，Apache-2.0，2026-08 观察）。

## 一、它做了什么

WrenAI 解决一个很实际的问题：**陌生 AI agent 落地到用户环境时，怎么知道有你这个工具、怎么学会用它？**

答案是 ~50 行的 **Discovery stub**（`npx skills add Canner/WrenAI`）：

1. 自动识别当前 agent 客户端（Claude Code / Cursor / Cline / Codex）
2. 装一个引导文件，只教会 agent 一件事：**需要时按需拉取**——`wren skills get <name>`
3. 拉到的内容永远钉在当前版本上（*content always matches the installed version*），不存在「文档落后于实现」

核心不是文档，是**机制**：引导文件极小、只指路，真实工作流按需取、随版本走。

## 二、对 uenv 的对应关系

`uenv doctor` 的产出已经具备 WrenAI 的另一半——**带提示的结构化错误**：

| 已有 | 对应 |
|---|---|
| `[Warning] 现象/原因/建议(Confirm)/回滚` 结构化输出 | WrenAI 的 structured errors with hints（弱同构，已实现） |
| 26 个 detector、三层（Host/Toolchain/Project） | — |

缺的是**「agent 发现 uenv」这一半**——目前只有人知道 `uenv doctor` 存在，agent 不知道。

## 三、可落地的形态（落地情况）

```
场景：Claude Code / Codex 进入一个陌生项目目录，要跑起来
机制：项目里有一个极小的 stub（脚本或 .claude/ 里的引导文件）
  ✓ 提示先跑 `uenv doctor --project . --agent`（只读、安全、产出结构化 JSON）——`uenv stub` 已实现
  → 自动识别 agent 客户端（Claude Code / Cursor / Cline / Codex 分形态）—— 未做，留中线
  → 需要时 `uenv skills get <category>` 拉某类环境的修复工作流（版本钉住）—— 未做，留中线
```

`uenv stub` 当前做到「stub 只指路 + 版本钉住」：
`uenv stub` 打印（或 `--out` 写入）一段含 `uenv v{version}` 的引导文本，教 agent 何时、如何调
`uenv doctor --agent`，并提示版本不同时用 `uenv stub --out` 重新生成——杜绝「文档先于实现飘走」。

与本象协议侧已有先例同构：新会话 AI 开工前先跑 `scripts/credential-map.mjs`——把「每会话重复摸索」变成「一个命令读完机器状态」。`uenv doctor` 就是这个思路的通用化（不只登录态，而是整台机器的可运行性）。

## 四、还没做 & 为什么（避免过度设计）

- **客户端自动检测 + 多类别按需拉取**：`uenv stub` 是通用文本，不针对 Claude Code/Cursor/Codex
  分形态；`uenv skills get <category>` 多类别工作流拉取也未做——agent 侧 skill 机制都还在快速变化，
  现在钉形态是过度设计。这两件留到中线「Agent Skill」。
- 「版本钉住、按需取用」这条原则**已经兑现**：stub 文本由 `uenv stub` 生成、编译期钉版本，
  任何写进 README/报告的修复建议都必须和 `uenv` 当前版本实际产出一致。

## 五、四层 stub 体系（全协议收尾，2026-08-07）

WrenAI 的 Discovery stub 不是一个工具的小技巧，而是本系四个协议的通用上手机制。
同一个病（agent 发现不了我们）、同一个药（极简 stub 指路 + 按需取当前版本真值），叠成四层：

| 层 | 落在哪 | stub 是什么 | 指路到哪 |
|---|---|---|---|
| **机器级** | uking 环境 | 全局 `~/.claude/CLAUDE.md` 用户偏好区一条指针 | `uenv doctor --agent` + `~/.uking/llms.txt` 按需读 |
| **项目级** | 本境（uenv） | `uenv stub` 生成的引导文本（版本钉住） | `uenv doctor --project . --agent` |
| **协议级** | 本象（Origin） | 仓库 `.claude/CLAUDE.md` | `credential-map.mjs --md` 先行 → `origin limits` → `test:conformance` |
| **动作级** | 影核（ActionParity） | 按需取单动作 schema | open365 `registry.ps1` 的 `get-action <name>`（版本钉住） |

**动作级说明**：影核已在 `~/.uking/tools/open365/core/` 实现（action-core.ps1 / registry.ps1，
ActionParity v0.1.0），但 `~/.uking/llms-full.txt` 全量预载约 40KB 动作 schema——
这正违反「按需取」：agent 不该为用一两个动作读进全部 schema。方向是改成像 WrenAI
`wren skills get <name>` 一样，`get-action <name>` 按需取单个动作的
参数/前置条件/回退，且随影核版本钉住。这同时是**本象协议草案 §九未决问题 Q4
「与影核协议动作 Schema 的正式对接方式」的答案方向**：对接接口就是「按名取 schema」，
不是把全部动作塞进上下文。

open365 是生产运行时，**先只记录方向，不动它**；落地时走 U-King 侧改造。
