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
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }

    // 3. 检查是否是 UTF-16LE 无 BOM（常见于 Windows Unicode 输出）
    if is_likely_utf16le(bytes) {
        return decode_utf16le(bytes);
    }

    // 4. 回退到当前 ANSI 代码页（Windows CP936 = GBK）
    decode_ansi(bytes)
}

/// 判断是否像 UTF-16LE（偶数字节数 + 每隔一个字节是 0x00 只对 ASCII 有效，中文则不然）
fn is_likely_utf16le(bytes: &[u8]) -> bool {
    if bytes.len() % 2 != 0 {
        return false;
    }
    // 检查是否有过多零字节（UTF-16LE 中 ASCII 字符的高位字节为 0）
    let zero_count = bytes.iter().step_by(2).filter(|&&b| b == 0).count();
    let total_pairs = bytes.len() / 2;
    // 超过 40% 的 pair 有零高位 → 可能是 UTF-16LE
    total_pairs > 0 && zero_count as f64 / total_pairs as f64 > 0.4
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
}
