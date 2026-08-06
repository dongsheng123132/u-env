# u-env（本境协议参考实现）

> **一条命令找出 Windows 项目为什么跑不起来。**
>
> `uenv doctor` —— 扫描你的开发环境，告诉你「这台机器能不能跑这个项目」，差在哪，怎么修。

```
$ uenv doctor --project .

[Warning] 4 条：
  [node.multiple-in-path(Warning)] PATH 里有多个 Node.js
    现象/原因：PATH 命中了 2 个 node.exe。你敲 `node` 时到底用哪个，完全取决于 PATH 顺序……
    建议（Confirm）：列出 PATH 中全部 node 位置，确认后手动移除多余项
      $ where node
      $ powershell -NoProfile -Command "(Get-Command node -All).Source"
  [rust.multiple-cargo-in-path(Warning)] PATH 里有多个 cargo
  [fs.project-path-non-ascii(Warning)] 项目路径含非 ASCII 字符
  [git.autocrlf-true(Warning)] git core.autocrlf=true 对 Rust 项目有风险

[Info] 3 条：
  [path.duplicate-entries(Info)] PATH 里有重复条目（21 组！）
  [path.missing-entries(Info)] PATH 里有不存在的目录（13 个！）
  [security.defender-scans-project(Info)] Defender 实时保护未排除项目目录

总结：0 error / 4 warning / 3 info
```

（以上是 `uenv doctor` 在本仓库机器上的真实输出——这些问题是真存在的。）

## 安装

当前为开发阶段，从源码构建：

```bash
git clone <本仓库>
cd u-env
cargo build --release
# 二进制在 target/release/uenv.exe，加入 PATH 即可
```

## 快速开始

```bash
# 扫描当前项目环境（生成 environment.origin.json）
uenv scan --project . --out environment.origin.json

# 诊断：这台机器能不能跑这个项目
uenv doctor --project .

# 环境指纹（同一台机器的有效环境两次扫描 hash 必须相同）
uenv fingerprint

# 好机器 vs 坏机器，直接指出差在哪
uenv diff environment.origin.json fixtures/env-broken.json

# 生成人读报告
uenv report --format markdown --project . --out report.md
```

给 AI 用的精简模式（JSON、无交互、失败即退出 1）：

```bash
uenv doctor --project . --agent
```

### 让 agent 发现 uenv（Discovery stub）

agent 落地陌生项目时不知道 `uenv doctor` 存在。把 stub 放进项目（或装到 agent 的全局配置），
它就会在需要判断「这机器能不能跑」时自动调用 uenv：

```bash
uenv stub                        # 打印 stub（版本钉住，可直接给 agent 读）
uenv stub --out .claude/uenv.md  # 写进项目
```

stub 只指路、不含实现细节，真值按需取、随 uenv 版本走。

## 支持矩阵

| 框架 | 扫描 | 诊断 | 修复 | 故障舱 | 重现 |
|---|---:|---:|---:|---:|---:|
| Tauri | ✅ | ✅ | 计划中 | 计划中 | 计划中 |
| Electron | ✅ | ✅ | 计划中 | 计划中 | 计划中 |
| Node.js | ✅ | ✅ | 计划中 | 计划中 | 计划中 |
| WinUI/.NET | Beta | Beta | — | 计划中 | — |

> ⚠️ 实事求是：没实现的都写「计划中」。目前支持 Windows 10/11 上的
> Tauri / Electron / Node 项目的环境扫描与诊断，其余框架在逐步扩展。

## 理念：本境协议（Origin Environment Protocol）

**代码定义软件做什么，本境定义软件在哪个世界里必然能工作。**

AI 时代的问题不再是「代码怎么写」，而是「这份代码在什么环境下能跑」。
本境协议把「环境」变成可扫描、可比对、可重现的对象：

- **Environment** —— 一次 scan 的产物（`environment.origin.json`）
- **Fingerprint** —— 环境的状态哈希，好机器 vs 坏机器一 diff 就知道差在哪
- **Finding + Fix** —— 诊断结果带可执行的修复建议
- **Capsule** —— （规划中）把出错现场固化成可重现的故障舱

> 协议草案见 `docs/protocol-v0.0.1.md`（由实现归纳而来，随实现演进）。

## 项目结构

```
crates/
├── uenv-core/        # 数据模型（Environment/Detector/Finding/Fingerprint）+ JSON Schema 导出
├── uenv-scanner/     # 26 个 detector：Host 10 + Toolchain 9 + Project 5 + 样板 2
├── uenv-rules/       # 规则引擎 + 24 条诊断规则
├── uenv-adapters/    # 框架适配器（tauri/electron/node）
├── uenv-fingerprint/ # 规范化 + 指纹 + diff
├── uenv-report/      # markdown/json 报告
└── uenv-cli/         # 命令行入口（scan/doctor/report/fingerprint/diff）
adapters/             # 适配器源码
schemas/              # environment.schema.json（由 uenv-core 导出）
```

## 贡献

提交一个你遇到过的 Windows Bug，让它以后永远不再困扰其他人。
五种贡献入口（Detector / Rule / Adapter / Capsule / Recipe）见 `CONTRIBUTING.md`。

## 路线图

- **阶段一（当前）**：Windows 环境医生 —— 扫描、诊断、修复建议（Tauri/Electron/Node）
- **阶段二**：故障重现网络 —— Capsule 故障舱、Recipe 复现步骤
- **阶段三**：本境协议独立化 —— 多 agent 共享的环境状态层

## 许可

Apache-2.0（代码）· CC BY 4.0（文档，见 `docs/LICENSE-DOCS`）

[English](README.en.md)
