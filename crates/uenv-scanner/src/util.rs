use std::env;
use std::path::PathBuf;

/// 解码子进程输出：BOM → 尝试 UTF-8 → 回退当前 ANSI 代码页。
/// 解不出的字节用 U+FFFD，不 panic。
pub fn decode_process_output(stdout: &[u8], stderr: &[u8]) -> (String, String) {
    (decode_bytes(stdout), decode_bytes(stderr))
}

fn decode_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }

    // 1. BOM 检测：UTF-16LE
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        return decode_utf16le(&bytes[2..]);
    }
    // UTF-16BE BOM
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        return decode_utf16be(&bytes[2..]);
    }
    // UTF-8 BOM
    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        return String::from_utf8_lossy(&bytes[3..]).to_string();
    }

    // 2. 尝试 UTF-8 严格解码
    // ⚠️ 含 NUL 的字节流（如纯 ASCII 的 UTF-16LE：" \x00 \x00N\x00A\x00"）也是
    //    合法 UTF-8，from_utf8 会直接成功 → 永远走不到 UTF-16 启发分支。
    //    先检查 NUL：有 NUL 就先做 UTF-16 判定，再决定用哪个解码器。
    if let Ok(s) = std::str::from_utf8(bytes) {
        if bytes.contains(&0) {
            // 可能是 UTF-16LE（ASCII 字符高位为 0）——wsl -l -v 表头/发行版名
            // 全是 ASCII，中文系统上一样输出纯 ASCII 的 UTF-16LE
            if is_likely_utf16le(bytes) {
                return decode_utf16le(bytes);
            }
        }
        return s.to_string();
    }

    // 3. 检查是否是 UTF-16LE 无 BOM（常见于 Windows Unicode 输出）
    if is_likely_utf16le(bytes) {
        return decode_utf16le(bytes);
    }

    // 4. 回退到当前 ANSI 代码页（Windows CP936 = GBK）
    decode_ansi(bytes)
}

/// 判断是否像 UTF-16LE（偶数字节数 + 高位字节（奇数位）多为 0x00）。
/// ⚠️ 必须检查**奇数位**（高位字节）：UTF-16LE 中 ASCII 字符是 `XX 00`，
/// 低位非零、高位为零。T0 曾误用 `step_by(2)` 检查低位，导致 wsl.exe 的
/// 无 BOM UTF-16LE 输出永远判 false → 走 GBK 解码 → 乱码。
fn is_likely_utf16le(bytes: &[u8]) -> bool {
    if bytes.len() % 2 != 0 {
        return false;
    }
    let total_pairs = bytes.len() / 2;
    if total_pairs == 0 {
        return false;
    }
    // 高位字节：位置 1, 3, 5, ...（ASCII 字符的高位为 0，中文字符的高位非 0）
    let zero_high = bytes.iter().skip(1).step_by(2).filter(|&&b| b == 0).count();
    // 超过 40% 的 pair 有零高位 → 可能是 UTF-16LE。
    // 中文系统上 wsl --status 输出"默认版本: 2"这类中英混合文本，
    // 中文部分高位非零，ASCII 部分高位为零，40% 阈值可区分。
    zero_high as f64 / total_pairs as f64 > 0.4
}

fn decode_utf16le(bytes: &[u8]) -> String {
    let u16s: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&u16s)
}

fn decode_utf16be(bytes: &[u8]) -> String {
    let u16s: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&u16s)
}

/// 用当前系统 ANSI 代码页解码（中文 Windows = CP936/GBK）
fn decode_ansi(bytes: &[u8]) -> String {
    // 使用 encoding_rs 获取系统 ANSI 编码
    // Windows 上 encoding_rs 可以按代码页号解码
    let ansi_cp = get_ansi_code_page();
    decode_with_code_page(bytes, ansi_cp)
}

fn get_ansi_code_page() -> u16 {
    // 通过 Windows API GetACP 或从 encoding_rs 检测
    // encoding_rs 不直接暴露 GetACP，我们通过尝试常见中文代码页
    // 实际上中文 Windows 就是 936(GBK)
    936
}

fn decode_with_code_page(bytes: &[u8], cp: u16) -> String {
    let label = match cp {
        936 => "gbk",
        950 => "big5",
        932 => "shift_jis",
        949 => "euc-kr",
        _ => "windows-1252",
    };
    match encoding_rs::Encoding::for_label(label.as_bytes()) {
        Some(enc) => {
            let (cow, _encoding, had_errors) = enc.decode(bytes);
            if had_errors {
                // 有解不出的字节，已用 U+FFFD 替换
                cow.into_owned()
            } else {
                cow.into_owned()
            }
        }
        None => String::from_utf8_lossy(bytes).to_string(),
    }
}

/// PATH 中查找同名可执行文件的所有命中（按 PATHEXT 展开），保序、去重。
pub fn find_all_in_path(exe: &str) -> Vec<PathBuf> {
    let pathext = env::var("PATHEXT")
        .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD;.VBS;.VBE;.JS;.JSE;.WSF;.WSH;.MSC".to_string());

    let extensions: Vec<&str> = pathext
        .split(';')
        .map(|e| e.trim())
        .filter(|e| !e.is_empty())
        .collect();

    let path_var = match env::var("PATH") {
        Ok(p) => p,
        Err(_) => return vec![],
    };

    let mut seen = std::collections::HashSet::new();
    let mut results = Vec::new();

    // 如果 exe 已经有扩展名，只用 exe 本身
    let has_ext = exe.contains('.');

    for dir in path_var.split(';') {
        let dir = dir.trim();
        if dir.is_empty() {
            continue;
        }

        if has_ext {
            let full = PathBuf::from(dir).join(exe);
            if full.is_file() {
                let canonical = canonicalize_lower(&full);
                if seen.insert(canonical.clone()) {
                    results.push(full);
                }
            }
        } else {
            for ext in &extensions {
                let name = format!("{exe}{ext}");
                let full = PathBuf::from(dir).join(&name);
                if full.is_file() {
                    let canonical = canonicalize_lower(&full);
                    if seen.insert(canonical) {
                        results.push(full);
                    }
                }
            }
        }
    }

    results
}

/// 规范化路径用于去重：全小写比较（Windows 大小写不敏感）
use std::path::Path;

fn canonicalize_lower(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_utf8_ascii() {
        let result = decode_bytes(b"hello world");
        assert_eq!(result, "hello world");
    }

    #[test]
    fn decode_utf8_chinese() {
        // "你好" in UTF-8
        let result = decode_bytes(&[0xE4, 0xBD, 0xA0, 0xE5, 0xA5, 0xBD]);
        assert_eq!(result, "你好");
    }

    #[test]
    fn decode_utf16le_bom() {
        // BOM + "A" in UTF-16LE
        let bytes = [0xFF, 0xFE, 0x41, 0x00];
        let result = decode_bytes(&bytes);
        assert_eq!(result, "A");
    }

    #[test]
    fn decode_utf8_with_bom() {
        // UTF-8 BOM + "test"
        let bytes = [0xEF, 0xBB, 0xBF, 0x74, 0x65, 0x73, 0x74];
        let result = decode_bytes(&bytes);
        assert_eq!(result, "test");
    }

    #[test]
    fn decode_empty() {
        assert_eq!(decode_bytes(b""), "");
    }

    /// wsl.exe 的输出是无 BOM 的 UTF-16LE（中英混合）。
    /// "默认版本: 2\r\n" 的 UTF-16LE 字节（无 BOM）→ 必须解出中文而不是 GBK 乱码。
    #[test]
    fn decode_wsl_utf16le_no_bom_mixed() {
        // 默认版本: 2\r\n  的 UTF-16LE 编码（无 BOM）
        let mut bytes = Vec::new();
        for ch in "默认版本: 2\r\n".encode_utf16() {
            bytes.extend_from_slice(&ch.to_le_bytes());
        }
        let result = decode_bytes(&bytes);
        assert_eq!(result, "默认版本: 2\r\n");
    }

    /// wsl --list --verbose 表头是纯 ASCII 的 UTF-16LE（含 NUL）。
    /// 这种字节流恰好是合法 UTF-8（NUL 是合法码点），不能走 from_utf8 分支。
    #[test]
    fn decode_ascii_utf16le_no_bom() {
        // "* Ubuntu    Stopped         2\r\n" 的 UTF-16LE 编码（无 BOM）
        let mut bytes = Vec::new();
        for ch in "* Ubuntu    Stopped         2\r\n".encode_utf16() {
            bytes.extend_from_slice(&ch.to_le_bytes());
        }
        let result = decode_bytes(&bytes);
        assert_eq!(result, "* Ubuntu    Stopped         2\r\n");
    }
}
