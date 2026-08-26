# DECISIONS.md — u-env 实现中的待拍板决策

> 每个决策点记录：背景 → 暂时采用的做法 → 需要人拍板的点。
> 每个决策先过一行站问句：**本决策是否引入新的「引用」？其解析器与新鲜度约束是什么？**
> （引用优先原则，见本象协议 `docs/09-引用优先-Reference-First.md`）

---

## D-001 encoding_rs 不在白名单但必须引入

- 背景：规格 §4 要求处理 GBK(CP936)/UTF-8/UTF-16 子进程输出解码。中国 Windows 机器 chcp 936 是常态（cmd / PowerShell 输出默认 GBK）。标准库 `String::from_utf8 / from_utf8_lossy` 只处理 UTF-8，`OsString` 不能用于任意 GBK 字节流解码。
- 我暂时采用的做法：引入 `encoding_rs`（Mozilla 维护的 Web 编码标准 Rust 实现，事实标准），在 `decode_ansi()` 中按代码页 936→gbk 标签解码。无额外依赖传递。
- 需要人拍板的点：是否接受 `encoding_rs` 加入白名单？或者有替代方案（如调用 Windows API `MultiByteToWideChar` 但需引入 windows-sys）？
- **✅ 裁决（Claude，2026-08-05）：批准，已加入规格 §1 白名单。** 理由：encoding_rs 是 Mozilla 维护的 WHATWG Encoding 标准实现、Firefox 在用、传递依赖仅 `cfg-if`；替代方案 `windows-sys` 引入的 API 面大得多，为了一个解码功能不划算。中文 Windows 上 GBK 解码是刚需，不是可选项。

---

## D-002 unicode-normalization 不在白名单，指纹 NFC 归一化未做

- 背景：规格 §5.1 要求 `Str` 规范化含 Unicode NFC。依赖白名单里没有 `unicode-normalization` crate，任务书 T4 明确指示"先只做 ASCII 安全的 trim + 空白折叠，把 NFC 记进 DECISIONS 作为待批依赖，不要偷偷加依赖"。
- 我暂时采用的做法：只做 ASCII 安全的 trim + 空白折叠（`fold_whitespace`）。中文全角/半角、组合字符（é vs e+́）在指纹中会被视为不同——对 Windows 开发环境（路径/版本号几乎全 ASCII）实际影响极小，因为事实值里中文字符串极少且不参与关键键。
- 需要人拍板的点：是否接受 `unicode-normalization`（`unicode-ident` 同作者，零传递依赖）加入白名单完成 NFC？或者维持现状（NFC 只在中文路径等场景有差异，风险可接受）？

---

## D-003 Agent Plugins 1.0.0 采纳为 skill 打包的「引用」

- 背景：Google 加入 Agent Plugins TSC（与 Amazon/Cursor/Microsoft/OpenAI/Vercel 同席），2026-08-06 发布 1.0.0 —— 开放、厂商中立的「Agent Skills + MCP server 打包成可移植插件」规范，核心主张「插件就是一个目录」（`plugin.json` + `skills/` + `mcp.json`，客户端专属扩展走反向域名目录）。Gate 0 实证（`research/gate0-agent-plugins/REPORT-gate0.md`）：uking-office 组 7 个 skill **原样**套一层 `plugin.json`，官方一致性校验 `agent-plugin-ts@0.1.1` 直接接受、0 条 SHOULD 报告——组件早已可移植，瓶颈在盒子，而这个盒子用共享规范、不用重造。但暴露内容层问题：5/7 个 SKILL.md 硬编码本机路径 `~/.uking/skills/...`（10 处），**规范合法 ≠ 可移植**。
- 站问句答案：**是**，本决策引入新「引用」= Agent Plugins 1.0.0。解析器 = `https://agent-plugins.org/schemas/1.0.0/plugin.schema.json`（`$schema` 是 const，版本即判别器，还有 `mcp.schema.json`）；新鲜度约束 = 规范当前是 Working Draft，随版本化 schema 演进（1.0.x 内 schema URL 不变，主版本变更即新引用需复审）；内容层路径修复（F1）是我们自持的产出，不属引用的新鲜度范围。
- 暂时采用的做法：把 Agent Plugins 1.0.0 登记为 skill 打包的引用标准，`plugin.json` 结构直接照抄（Gate 0 已验证），不改动 uenv 核心。
- 需要人拍板的点：
  1. 是否把 Agent Plugins 定为 skill 供应链的**正式**打包规范（影响 clawhub/skillhub 分发格式）？还是仅登记为参考、暂不绑定？
  2. F1 可移植性修复（SKILL.md 硬编码路径改相对插件根）是否立项？—— 建议立项：成本低，office-read/office-edit 两个路径干净的 skill 是现成样板。
  3. 后续 uenv 若往 Describe 层（能力目录：把 skills/MCP 枚举进环境快照）探，是否以 ARD/Plugin 为对齐目标？—— 建议暂缓，等 #1 拍板后再定。
- **✅ 裁决（Claude 受用户委托拍板，2026-08-07）**：
  1. **暂不绑定**为正式分发规范 —— 1.0.0 仍是 Working Draft，保持"登记为引用"；待转稳定版后复审。
  2. **F1 立项并已在插件副本试点完成**（`research/gate0-agent-plugins/uking-office-suite/`）：10 处硬编码路径全改可移植约定（`<本 SKILL.md 同目录>/scripts/`），零依赖 + 官方 `agent-plugin-ts@0.1.1` 双校验通过、0 条可移植性警告。**运行中的 `~/.claude/skills` 与 `~/.uking/skills` 暂不改**——改动活体工具前需先做一次运行时冒烟（在 Claude Code 里真跑一个 skill），确认相对路径解析 OK 再全量铺。
  3. **暂缓** ARD 对齐 —— 等 uenv 真立项能力目录（Describe 层）时再议。

