# Gate 0 实验报告 · Agent Plugins 包装层试水（uking-office）

> 日期：2026-08-07 · 状态：**GO（包装层成立；F1 可移植性问题已在插件副本修复并复验）**
> 起因：Google 加入 Agent Plugins TSC 发布 1.0.0（`plugin.json + skills/ + mcp.json`），
> 问"我们能做点啥" → 选成本最低的路线：把现成 skill 组打成合法插件，验证包装层。
> 拍板：DECISIONS D-003（受用户委托）——登记为引用、暂不绑定；F1 立项；ARD 对齐暂缓。

## 1. 方法与装置

| 项 | 设置 |
|---|---|
| 被测插件 | `uking-office-suite/` —— uking-office 组 7 个 skill 原样复制（SKILL.md + scripts/），**内容一字未改** |
| 权威校验 | `agent-plugin-ts@0.1.1`（官方 TS 实现，`loadPluginFromDirectory`） |
| 冒烟校验 | `validate.mjs`（零依赖 node，结构规则 + 可移植性扫描） |
| skill 来源 | `~/.uking/skills/` 与 `~/.claude/skills/` 逐文件 diff 相同，取前者 |
| 打包对象 | 7 个：uking-docx / uking-mail / uking-office-edit / uking-office-read / uking-pdf / uking-ppt / uking-xlsx |

## 2. 结果

### 官方一致性校验（agent-plugin-ts@0.1.1）

```
✅ PLUGIN ACCEPTED
   name:    uking-office  v0.1.0
   skills:  7 个（全部加载，description 完整解析）
   mcp:     absent（本组无 MCP server，合法）
   diagnostics（SHOULD 级）: 0 条
```

### 零依赖冒烟校验（validate.mjs）

- 结构：**全部通过**（plugin.json $schema 常量 / name pattern / 无未知字段 / 7/7 skill 目录含合法 SKILL.md frontmatter）
- 可移植性警告：**5 条，10 处**硬编码 `~/.uking/skills/<skill>/scripts/<script>.mjs` 本机路径

### F1 修复试点（插件副本，2026-08-07）

把 10 处 `~/.uking/skills/<skill>/scripts/` 改为可移植约定 `<本 SKILL.md 同目录>/scripts/`，
并在每个 SKILL.md 核心用法处补一行"脚本随本 skill 分发"说明。复验：

- 零依赖校验：结构全过，**可移植性警告 5→0 条**
- 官方一致性校验：**ACCEPTED**，7 skill、0 diagnostics
- 残留扫描：SKILL.md 中 `~/.uking` / `C:\` / `D:\` 一处不剩

**活体 skill 未动**（`~/.claude/skills` 与 `~/.uking/skills`）：相对路径约定在 Claude Code 里
能否稳定解析需先做一次运行时冒烟，确认后再全量铺，避免未验证就改运行中工具（见 DECISIONS D-003 裁决 2）。

## 3. 结论：Gate 0 GO

**包装层零成本成立**。我们的 skill 本来就是合法 Agent Skills，套一层 `plugin.json` 就是规范合法的
Agent Plugin，官方一致性实现直接接受。这印证了文章的核心论点——组件（skill）早已可移植，瓶颈在
"盒子"；而这个盒子我们用上了共享规范，没自造。

**真正的成本不在包装层，在内容层**：`SKILL.md` 里把脚本路径写死成本机 `~/.uking/...`。
规范校验看不出来（路径不在 mcp.json 里，无围堵违规），但插件一到别的机器/别的客户端就指向虚空。
**"规范合法" ≠ "可移植"，这一步正是 uenv 该管的活**（见下）。

## 4. 发现

| # | 发现 | 严重度 | 处理方向 |
|---|---|---|---|
| F1 | 10 处 `~/.uking/...` 硬编码路径，5/7 skill 受影响（office-read/office-edit 干净） | 高 | **插件副本已修**（可移植约定 `<本 SKILL.md 同目录>/scripts/`，双校验通过）；活体 skill 待运行时冒烟后铺 |
| F2 | 本组无 MCP server（纯 node/python 脚本），`mcp.json` 缺席 | 低 | 合法；若日后有 MCP 化需求再加，`plugin.json` 不内联 MCP（规范明确） |
| F3 | 零依赖脚本 + SKILL.md 结构 = 插件已自洽 | — | 打包即装即用，无需安装器 |

## 5. 对 #2 / #3 的启示

- **#2（uenv 往 Describe 层探）**：可移植性正是"环境描述"能帮上的地方。若 uenv 把插件根目录也扫进
  facts（skills/ 列表、硬编码路径探测器），就能在 `uenv doctor` 里直接报 F1 这类"规范合法但不可移植"，
  把环境知识变成诊断——这跟浏览器 Gate 0 的"约束层"是同一逻辑。
- **#3（DECISIONS 引用登记）**：Agent Plugins 1.0.0 是一个应记入 `docs/DECISIONS.md` 的新「引用」：
  解析器 = `agent-plugins.org/schemas/1.0.0/plugin.schema.json`，新鲜度 = 跟随 Working Draft→1.0.0 稳定版。
- **可移植修法验证**：office-read / office-edit 两个 SKILL.md 不含任何本机路径，天然可移植，
  是"路径干净"的样板，值得做一次 F1 修复时对照。

## 6. 复现

```bash
# 官方一致性校验（临时目录装过一次，可重装）
npm install agent-plugin-ts && node run-validate.mjs uking-office-suite/

# 零依赖冒烟校验（无需 npm）—— 含可移植性扫描，应保持 0 条警告
node validate.mjs uking-office-suite/
```

待办（未动活体 skill 的原因）：
1. 运行时冒烟 —— 在 Claude Code 里真跑一次 `uking-office-suite` 里某 skill（如 uking-xlsx），
   确认 `<本 SKILL.md 同目录>` 相对路径约定能解析；
2. 冒烟过 → 把同一约定铺到 `~/.claude/skills` 与 `~/.uking/skills` 的 5 个源文件。

产物：`uking-office-suite/`（插件本体）· `validate.mjs`（零依赖校验器）· 本报告
