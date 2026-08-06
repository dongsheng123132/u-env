# u-env 环境报告

环境基本可用，但有 4 个 Warning 值得处理。

## 环境概览

| 项 | 值 |
|---|---|
| 系统 | Windows 11 Home build 22631 |
| 架构 | X64 |
| detector 数 | 26 |

## 问题

### Warning

#### `node.multiple-in-path` — PATH 里有多个 Node.js

PATH 命中了 2 个 node.exe。你敲 `node` 时到底用哪个，完全取决于 PATH 顺序——今天能用、明天在别的终端里就版本不对，这类「换了个终端就坏」的玄学 bug 十有八九是这个。常见来源：nvm 残留链接 + 全局安装的 Node + %APPDATA%\npm 路径同时存在。建议统一到一个版本管理器（nvm-windows / fnm / volta）管理。. 

**建议**（Confirm）：列出 PATH 中全部 node 位置，确认后手动移除多余项

```bash
where node
powershell -NoProfile -Command "(Get-Command node -All).Source"
```
<details><summary>回滚</summary>

```bash
（无自动回滚——只读命令，改动需手动在系统设置里撤）
```
</details>

#### `rust.multiple-cargo-in-path` — PATH 里有多个 cargo

PATH 命中了 2 个 cargo.exe。这比多 Node 更阴险：cargo 会按它自己编译时记录的路径调用 rustc，两个 cargo 通常各带一个 rustc，版本不一致时 `cargo build` 和 `rustc --version` 说的根本不是一回事，增量编译缓存（target/）也会互相污染。rustup 管理的唯一正确姿势是只用 `~/.cargo/bin` 里的 cargo，其它全删。. 

**建议**（Confirm）：列出 PATH 中全部 cargo 位置，确认后保留 rustup 的 ~/.cargo/bin 一份

```bash
where cargo
rustup show active-toolchain
```
<details><summary>回滚</summary>

```bash
（无自动回滚——只读命令，改动需手动在系统设置里撤）
```
</details>

#### `fs.project-path-non-ascii` — 项目路径含非 ASCII 字符

项目路径包含中文等非 ASCII 字符。虽然现代工具大多支持 UTF-8 路径，但仍有大量老工具链/原生库按 ANSI 处理路径（尤其 MSVC 老版本、部分 npm 原生模块的编译脚本），会报「无法打开文件」或乱码路径错误。能改则改，至少心里有数。. 

**建议**（Manual）：将项目移到纯 ASCII 路径（如 C:\dev\proj）

```bash
echo "建议路径只含 a-zA-Z0-9-_"
```
<details><summary>回滚</summary>

```bash
（无法自动回滚——涉及项目迁移）
```
</details>

#### `git.autocrlf-true` — git core.autocrlf=true 对 Rust 项目有风险

core.autocrlf=true 会在 checkout 时把 LF 转 CRLF。Rust 项目（尤其带 .sh 脚本、Makefile、或者被 CI 拉取在 Linux 上构建的仓库）会因为行尾转换产生 diff 噪音甚至脚本执行错误。建议对仓库用 .gitattributes 显式声明，或对代码仓库设 autocrlf=input/false。. 

**建议**（Confirm）：对本仓库关闭 autocrlf 或改用 input

```bash
git config core.autocrlf input
```
<details><summary>回滚</summary>

```bash
git config core.autocrlf true
```
</details>

### Info

#### `path.duplicate-entries` — PATH 里有重复条目

同一个目录在 PATH 里出现了 21 组重复。重复本身不致命，但会让排查「你机器上到底哪个 exe 生效」变得更难——which 命中的顺序完全取决于 PATH 顺序，重复条目会掩盖 nvm/volta/fnm 之类的版本切换问题。建议顺手清理，尤其是安装多个工具链后残留的旧路径。. 

**建议**（Confirm）：清理 PATH 重复条目（用户级，去重后写回）

```bash
powershell -NoProfile -Command "$p=[Environment]::GetEnvironmentVariable('Path','User'); [Environment]::SetEnvironmentVariable('Path',(($p -split ';' | Select-Object -Unique) -join ';'),'User')"
```
<details><summary>回滚</summary>

```bash
powershell -NoProfile -Command "$p=[Environment]::GetEnvironmentVariable('Path','User'); [Environment]::SetEnvironmentVariable('Path',$p,'User')"
```
</details>

#### `path.missing-entries` — PATH 里有不存在的目录

13 个 PATH 条目指向不存在的目录。这通常是卸载软件后残留的旧路径（比如老版 Node、被删掉的工具目录）。本身不致命——Windows 会静默跳过，但会拖慢每次命令行启动（系统逐个 stat 这些路径），而且等你装回同名工具时行为可能出乎意料。建议定期清理。. 

**建议**（Confirm）：列出并移除 PATH 里不存在的目录（先预览再确认）

```bash
powershell -NoProfile -Command "$p=[Environment]::GetEnvironmentVariable('Path','User'); $keep=$p -split ';' | Where-Object { $_ -and (Test-Path $_) }; [Environment]::SetEnvironmentVariable('Path',($keep -join ';'),'User')"
```
<details><summary>回滚</summary>

```bash
powershell -NoProfile -Command "$p=[Environment]::GetEnvironmentVariable('Path','User'); [Environment]::SetEnvironmentVariable('Path',$p,'User')"
```
</details>

#### `security.defender-scans-project` — Defender 实时保护未排除项目目录

Defender 实时保护开着，但排除项没覆盖项目目录。node_modules/cargo target 这类海量小文件每次构建都被实时扫描，编译时间可能慢 30-50%（尤其 Rust 增量编译）。把项目目录和工具链目录加进 Defender 排除项能显著提速——注意只排除你信任的开发目录。. 

**建议**（Manual）：把项目目录加入 Defender 排除项（需管理员）

```bash
powershell -NoProfile -Command "Add-MpPreference -ExclusionPath '<项目路径>'"
```
<details><summary>回滚</summary>

```bash
powershell -NoProfile -Command "Remove-MpPreference -ExclusionPath '<项目路径>'"
```
</details>

## 环境指纹

- host: `origin-env:sha256:2032fcf56217f73fd4583c73107b62517e7769b6a5282ecd541d896cd503d889`
- toolchain: `origin-env:sha256:6106f7d422222c87dece97ad9b4b0503147dbd83b383249661267aba3b5fbafc`
- project: `origin-env:sha256:f3b9d16d23bfcb61eeb67cd181ae262f2447ac08d4548fdd152186ed1b5aab42`
- full: `origin-env:sha256:c383b5a24ece574742235c48f2bdac7ae937f09a088085f433f0509ca226c5dc`

## 附录：全部 detector

| detector | 状态 | summary |
|---|---|---|
| `fs.project-location` | ok | D: on NTFS (D:\uking编程\本境协议) |
| `host.disk` | ok | 5 个卷 |
| `host.hardware` | ok | 12th Gen Intel(R) Core(TM) i9-12900H |
| `net.proxy` | ok | 系统代理 127.0.0.1:7897，与 env 一致 |
| `path.analysis` | ok | PATH 93 条 |
| `project.drift` | ok | 1 个工具声明比对 |
| `project.git` | ok | master (dirty) |
| `project.kind` | ok | rust |
| `project.lockfiles` | ok | 1 个 lockfile |
| `project.manifests` | ok | 1 个声明 |
| `runtime.webview2` | ok | WebView2 Runtime 120.0.2210.144 (evergreen) |
| `security.defender` | degraded | 实时保护 开启，排除项不可读（需管理员） |
| `toolchain.dotnet` | ok | 1 个 SDK |
| `toolchain.git` | ok | Git 2.49.0.windows.1 |
| `toolchain.msvc` | ok | 1 个实例（含 C++ workload） |
| `toolchain.node` | ok | Node.js 2 installation(s), version Version("22.14.0") |
| `toolchain.npm-family` | ok | npm 10.9.2 |
| `toolchain.python` | ok | Python 3.12.7 |
| `toolchain.rust` | ok | active stable-x86_64-pc-windows-msvc |
| `toolchain.windows-sdk` | ok | 1 个 SDK 版本，最新 10.0.19041.0 |
| `windows.developer-mode` | ok | 开发者模式已开启 |
| `windows.locale` | ok | ACP 936 / OEM 936 / locale zh-CN |
| `windows.long-paths` | ok | 长路径支持已开启 |
| `windows.powershell` | ok | Windows PowerShell 5.1.22621.6133 / PowerShell 7+ 7.6.4 |
| `windows.version` | ok | Windows 11 Home build 22631 |
| `wsl.status` | ok | WSL 已安装, 默认版本 2, 1 个发行版 |

