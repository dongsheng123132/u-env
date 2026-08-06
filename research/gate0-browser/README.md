# Gate 0 · 浏览器玩法三臂对比实验

> 立项方案：`../../docs/11-浏览器玩法-立项方案.md` §5 Phase 0
> 决策规则：快照+约束+证据包（本象包）在有效效率上**明显优于**纯文本/纯截图 → 立项继续；否则只做线 A。

## 两任务

- **任务 1**：`fixtures/order-form.html` —— 填表 → 校验总价正确 → 提交 → 验证跳转到确认页。
- **任务 2**：`fixtures/operable-page.html` —— 找出页面里所有可操作对象（按钮/链接/输入框/下拉）。

## 三臂

| 臂 | 输入给 agent 的形式 |
|---|---|
| A 纯文本 | 页面 a11y 快照/innerText 的纯文本 |
| B 纯截图 | 页面截图（PNG） |
| C 本象包 | 结构化 web 快照（URL/标题/DOM 事实/表单态/console）+ 约束声明 + 证据 |

## 执行与判分

- 执行 agent：hermes（deepseek-v4-flash 廉价通道），批量跑。
- 判分 agent：**只用 qwen-plus**（deepseek-v4-flash 同题判分全判 no，LongMemEval 教训）。

## 进度

- [x] **CDP 连接冒烟测试**（2026-08-07）：无头 Chrome 150 / CDP 1.3，Node 零依赖 WebSocket 直连成功，中文路径 file URL 正常。`smoke-cdp.mjs`
- [x] **线 B 动作链 + parity 校验**（2026-08-07）：`act-fill-order.mjs` 全流程跑通——填 2/3/1 → 总价自动算 43 → 提交 → 确认页可见 + hash `#confirmed` + 金额一致，两条 parity 全 PASS。**线 B 可行性已实证，非纸面设计**
- [x] **三臂渲染管道**：`render-arms.mjs` → `runs/<task>/arms/`（A 文本 / B 截图 / C 本象包）
- [x] **批量执行 + 判分**：`run-gate0.mjs`（hermes 执行）+ `judge-gate0.mjs`（qwen-plus 判分）
- [x] **分析报告 → GO**：`REPORT-gate0.md`（Q3 验证 A 0% vs C 96%；枚举 A 33% vs C 100%）

## 里程碑记录

- 2026-08-07：CDP 驱动 + 填表动作链 + parity 校验全绿（Chrome 150 / Node 22 / 零依赖）。
- 2026-08-07：**Gate 0 GO** —— 协议层差异化实证（约束层让「验证」从不可能变可能，0%→96%；结构化事实让枚举 33%→100%）。B 臂截图待视觉通道补齐。
