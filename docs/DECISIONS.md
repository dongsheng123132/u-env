# DECISIONS.md — u-env 实现中的待拍板决策

> 每个决策点记录：背景 → 暂时采用的做法 → 需要人拍板的点。

---

## D-001 encoding_rs 不在白名单但必须引入

- 背景：规格 §4 要求处理 GBK(CP936)/UTF-8/UTF-16 子进程输出解码。中国 Windows 机器 chcp 936 是常态（cmd / PowerShell 输出默认 GBK）。标准库 `String::from_utf8 / from_utf8_lossy` 只处理 UTF-8，`OsString` 不能用于任意 GBK 字节流解码。
- 我暂时采用的做法：引入 `encoding_rs`（Mozilla 维护的 Web 编码标准 Rust 实现，事实标准），在 `decode_ansi()` 中按代码页 936→gbk 标签解码。无额外依赖传递。
- 需要人拍板的点：是否接受 `encoding_rs` 加入白名单？或者有替代方案（如调用 Windows API `MultiByteToWideChar` 但需引入 windows-sys）？
