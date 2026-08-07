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

