// 规范化（规格 §5.1）+ canonical JSON 写出（§5.2）。
//
// ⚠️ NFC 归一化：白名单里没有 unicode-normalization crate，本任务只做
// ASCII 安全的 trim + 空白折叠；NFC 已记进 docs/DECISIONS.md 待批。

use std::collections::BTreeMap;

use uenv_core::FactValue;

/// 规范化 FactValue。返回 None 表示规范化为空值（空 Str / 空 List / 空 Map），
/// 父级必须删除该键。
pub fn normalize_fact_value(fv: &FactValue) -> Option<FactValue> {
    match fv {
        FactValue::Str(s) => {
            let folded = fold_whitespace(s.trim());
            if folded.is_empty() {
                None
            } else {
                Some(FactValue::Str(folded))
            }
        }
        FactValue::Path(p) => {
            let norm = normalize_path(p);
            if norm.is_empty() {
                None
            } else {
                Some(FactValue::Path(norm))
            }
        }
        FactValue::Version(v) => {
            let t = v.trim();
            if t.is_empty() {
                None
            } else {
                Some(FactValue::Version(t.to_string()))
            }
        }
        FactValue::Int(_) | FactValue::Bool(_) => Some(fv.clone()),
        FactValue::List(items) => {
            let mut out = Vec::new();
            for item in items {
                if let Some(n) = normalize_fact_value(item) {
                    out.push(n);
                }
            }
            if out.is_empty() {
                None
            } else {
                Some(FactValue::List(out))
            }
        }
        FactValue::Set(items) => {
            // 先各自规范化，再按规范化后的 JSON 文本排序、去重
            let mut out: Vec<FactValue> = Vec::new();
            for item in items {
                if let Some(n) = normalize_fact_value(item) {
                    out.push(n);
                }
            }
            out.sort_by_cached_key(canonical_json);
            out.dedup_by(|a, b| canonical_json(a) == canonical_json(b));
            if out.is_empty() {
                None
            } else {
                Some(FactValue::Set(out))
            }
        }
        FactValue::Map(map) => {
            let mut out: BTreeMap<String, FactValue> = BTreeMap::new();
            for (k, v) in map {
                let k = fold_whitespace(k.trim());
                if k.is_empty() {
                    continue;
                }
                if let Some(n) = normalize_fact_value(v) {
                    out.insert(k, n);
                }
            }
            if out.is_empty() {
                None
            } else {
                Some(FactValue::Map(out))
            }
        }
    }
}

/// 空白折叠：连续空白（空格/tab/换行/CR）折叠成一个空格
fn fold_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_ws {
                out.push(' ');
                prev_ws = true;
            }
        } else {
            out.push(c);
            prev_ws = false;
        }
    }
    out
}

/// Path 规范化：反斜杠→正斜杠；盘符大写（c:/ → C:/）；去尾部 /；
/// 其余部分保留大小写；再走 Str 规则（trim + 空白折叠）。
pub fn normalize_path(p: &str) -> String {
    let mut s = p.replace('\\', "/");
    s = s.trim().trim_end_matches('/').to_string();
    // 盘符大写：C:/xxx → c:/xxx
    let bytes: Vec<char> = s.chars().collect();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == ':' {
        let drive = bytes[0].to_ascii_uppercase();
        s = format!("{drive}:{}", bytes[2..].iter().collect::<String>());
    }
    fold_whitespace(&s)
}

/// canonical JSON（规格 §5.2）：键排序（BTreeMap 保证）、无多余空白、
/// 整数不带小数点、不允许浮点。不用 serde_json::to_string —— 手动递归写出，
/// 保证格式确定性。FactValue 序列化为带类型标记的对象（与 serde 派生一致），
/// 这样 Version("1.88") 与 Str("1.88") 不会混淆。
pub fn canonical_json(fv: &FactValue) -> String {
    let mut out = String::new();
    write_fact_value(fv, &mut out);
    out
}

fn write_fact_value(fv: &FactValue, out: &mut String) {
    match fv {
        FactValue::Str(s) => write_tagged("str", s, out),
        FactValue::Int(i) => {
            out.push_str("{\"int\":");
            out.push_str(&i.to_string());
            out.push('}');
        }
        FactValue::Bool(b) => {
            out.push_str(if *b {
                "{\"bool\":true}"
            } else {
                "{\"bool\":false}"
            });
        }
        FactValue::Version(v) => write_tagged("version", v, out),
        FactValue::Path(p) => write_tagged("path", p, out),
        FactValue::List(items) => {
            out.push_str("{\"list\":[");
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_fact_value(item, out);
            }
            out.push_str("]}");
        }
        FactValue::Set(items) => {
            // 排序后写出（规范化时已排，这里再排是防御：canonical_json 可能被直接调用）
            let mut sorted: Vec<&FactValue> = items.iter().collect();
            sorted.sort_by_cached_key(|v| canonical_json(v));
            out.push_str("{\"set\":[");
            for (i, item) in sorted.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_fact_value(item, out);
            }
            out.push_str("]}");
        }
        FactValue::Map(map) => {
            out.push_str("{\"map\":{");
            for (i, (k, v)) in map.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_json_string(k, out);
                out.push(':');
                write_fact_value(v, out);
            }
            out.push_str("}}");
        }
    }
}

fn write_tagged(tag: &str, value: &str, out: &mut String) {
    out.push_str("{\"");
    out.push_str(tag);
    out.push_str("\":");
    write_json_string(value, out);
    out.push('}');
}

/// JSON 字符串转义（最小实现：引号/反斜杠/控制字符）
fn write_json_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> FactValue {
        FactValue::Str(v.to_string())
    }

    #[test]
    fn str_trim_and_fold() {
        assert_eq!(
            normalize_fact_value(&s("  hello   world  ")),
            Some(FactValue::Str("hello world".to_string()))
        );
        assert_eq!(normalize_fact_value(&s("   ")), None);
    }

    #[test]
    fn path_normalize() {
        let p = FactValue::Path(r"c:\Users\Me\AppData\".to_string());
        assert_eq!(
            normalize_fact_value(&p),
            Some(FactValue::Path("C:/Users/Me/AppData".to_string()))
        );
        // 盘符大写、反斜杠转正斜杠、去尾斜杠
        assert_eq!(normalize_path(r"d:\tools\node\"), "D:/tools/node");
        // 小写盘符转大写
        assert_eq!(normalize_path("e:/x"), "E:/x");
        // 相对路径不动
        assert_eq!(normalize_path("tools/node"), "tools/node");
    }

    #[test]
    fn version_kept_as_is() {
        // Version 原样保留：1.88.0 ≠ 1.88
        let v = FactValue::Version("1.88".to_string());
        assert_eq!(
            normalize_fact_value(&v),
            Some(FactValue::Version("1.88".to_string()))
        );
        assert_ne!(
            canonical_json(&FactValue::Version("1.88".to_string())),
            canonical_json(&FactValue::Version("1.88.0".to_string()))
        );
        // Version 与 Str 不混淆
        assert_ne!(
            canonical_json(&FactValue::Version("1.88".to_string())),
            canonical_json(&FactValue::Str("1.88".to_string()))
        );
    }

    #[test]
    fn set_sorted_dedup() {
        let set = FactValue::Set(vec![s("b"), s("a"), s("a"), s("c")]);
        let n = normalize_fact_value(&set).unwrap();
        match n {
            FactValue::Set(items) => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], FactValue::Str("a".to_string()));
                assert_eq!(items[1], FactValue::Str("b".to_string()));
                assert_eq!(items[2], FactValue::Str("c".to_string()));
            }
            _ => panic!("expected Set"),
        }
    }

    #[test]
    fn map_drops_empty_values() {
        let map = FactValue::Map(BTreeMap::from([
            ("keep".to_string(), s("v")),
            ("empty".to_string(), s("")),
            ("ws".to_string(), s("   ")),
            ("empty_list".to_string(), FactValue::List(vec![])),
            ("empty_map".to_string(), FactValue::Map(BTreeMap::new())),
        ]));
        let n = normalize_fact_value(&map).unwrap();
        match n {
            FactValue::Map(m) => {
                assert!(m.contains_key("keep"));
                assert!(!m.contains_key("empty"));
                assert!(!m.contains_key("ws"));
                assert!(!m.contains_key("empty_list"));
                assert!(!m.contains_key("empty_map"));
            }
            _ => panic!("expected Map"),
        }
    }

    #[test]
    fn canonical_json_deterministic() {
        let map = FactValue::Map(BTreeMap::from([
            ("z".to_string(), s("1")),
            ("a".to_string(), s("2")),
        ]));
        let j1 = canonical_json(&map);
        let j2 = canonical_json(&map);
        assert_eq!(j1, j2);
        // 键序确定（a 在前）
        assert!(j1.find("a").unwrap() < j1.find("z").unwrap());
    }

    #[test]
    fn canonical_int_no_decimal() {
        assert_eq!(canonical_json(&FactValue::Int(5)), "{\"int\":5}");
        assert_eq!(
            canonical_json(&FactValue::Int(-123456)),
            "{\"int\":-123456}"
        );
    }
}
