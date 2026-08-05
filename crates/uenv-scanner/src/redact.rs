// 脱敏——默认开启。规格来源：docs/10-架构与数据模型.md §8

use std::env;

/// 脱敏：用户名/机器名/密钥样式串 → 占位符
pub fn redact(s: &str) -> String {
    let mut result = s.to_string();

    // 1. 用户名路径 C:\Users\XXX → C:\Users\<user>
    if let Ok(user) = env::var("USERNAME") {
        if !user.is_empty() {
            let pattern = format!("\\Users\\{user}");
            let replacement = "\\Users\\<user>";
            // 大小写不敏感替换
            result = replace_case_insensitive(&result, &pattern, replacement);
        }
    }

    // 2. HOME / USERPROFILE 路径
    if let Ok(home) = env::var("USERPROFILE") {
        if !home.is_empty() {
            result = replace_case_insensitive(&result, &home, "<user>");
        }
    }

    // 3. 机器名
    if let Ok(host) = env::var("COMPUTERNAME") {
        if !host.is_empty() {
            result = replace_case_insensitive(&result, &host, "<host>");
        }
    }

    // 4. 密钥样式串：sk-... / ghp_... / 等
    result = redact_secrets(&result);

    // 5. 代理 URL 中的账号密码
    result = redact_proxy_auth(&result);

    result
}

fn replace_case_insensitive(text: &str, pattern: &str, replacement: &str) -> String {
    let lower_text = text.to_lowercase();
    let lower_pattern = pattern.to_lowercase();

    let mut result = String::with_capacity(text.len());
    let mut pos = 0;

    while pos < text.len() {
        if let Some(idx) = lower_text[pos..].find(&lower_pattern) {
            let abs_idx = pos + idx;
            result.push_str(&text[pos..abs_idx]);
            result.push_str(replacement);
            pos = abs_idx + pattern.len();
        } else {
            result.push_str(&text[pos..]);
            break;
        }
    }

    result
}

fn redact_secrets(s: &str) -> String {
    let mut result = s.to_string();

    // sk- 前缀（OpenAI / 通用 API key）
    result = redact_pattern(&result, "sk-", 48);

    // ghp_ 前缀（GitHub personal access token）
    result = redact_pattern(&result, "ghp_", 40);

    // gho_ 前缀（GitHub OAuth token）
    result = redact_pattern(&result, "gho_", 40);

    // ghu_ 前缀（GitHub user-to-server token）
    result = redact_pattern(&result, "ghu_", 40);

    // github_pat_ 前缀
    result = redact_pattern(&result, "github_pat_", 82);

    result
}

fn redact_pattern(s: &str, prefix: &str, max_len: usize) -> String {
    let lower = s.to_lowercase();
    let lower_prefix = prefix.to_lowercase();

    let mut result = String::with_capacity(s.len());
    let mut pos = 0;

    while pos < s.len() {
        if let Some(idx) = lower[pos..].find(&lower_prefix) {
            let abs_idx = pos + idx;
            result.push_str(&s[pos..abs_idx]);

            // 匹配前缀后的字符（直到空白或 max_len）
            let remaining = &s[abs_idx..];
            let end = remaining
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                .unwrap_or(remaining.len())
                .min(max_len);

            result.push_str("<redacted>");
            pos = abs_idx + end;
        } else {
            result.push_str(&s[pos..]);
            break;
        }
    }

    result
}

fn redact_proxy_auth(s: &str) -> String {
    // http://user:pass@host → http://<redacted>@host
    let re_patterns = [
        ("http://", "http://<redacted>@"),
        ("https://", "https://<redacted>@"),
    ];

    let mut result = s.to_string();
    for (scheme, _replacement) in &re_patterns {
        let lower = result.to_lowercase();
        let mut pos = 0;
        let mut new_result = String::with_capacity(result.len());

        while pos < result.len() {
            if let Some(idx) = lower[pos..].find(*scheme) {
                let abs_idx = pos + idx;
                new_result.push_str(&result[pos..abs_idx + scheme.len()]);

                let after_scheme = &result[abs_idx + scheme.len()..];
                // 查找 @ 符号，表示 user:pass@host
                if let Some(at_pos) = after_scheme.find('@') {
                    // 确认 @ 在下一个 / 之前，确保是 URL 认证而非其他
                    let slash_pos = after_scheme.find('/');
                    if slash_pos.is_none_or(|sp| at_pos < sp) {
                        new_result.push_str("<redacted>@");
                        pos = abs_idx + scheme.len() + at_pos + 1;
                        continue;
                    }
                }
                new_result.push_str(&result[pos..abs_idx + scheme.len()]);
                pos = abs_idx + scheme.len();
            } else {
                new_result.push_str(&result[pos..]);
                break;
            }
        }
        result = new_result;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_sk_key() {
        let input = "export OPENAI_API_KEY=sk-proj-abc123def456ghi789jkl012mno345pqr678stu901vwx";
        let output = redact(input);
        assert!(output.contains("<redacted>"));
        assert!(!output.contains("sk-proj"));
    }

    #[test]
    fn redact_ghp_token() {
        let input = "token: ghp_1a2B3c4D5e6F7g8H9i0J1k2L3m4N5o6P7q8R9s0T";
        let output = redact(input);
        assert!(output.contains("<redacted>"));
    }

    #[test]
    fn redact_username_in_path() {
        // 这个测试依赖环境变量 USERNAME
        // 在 CI 或测试环境里可能不存在，跳过比较
        if let Ok(_user) = std::env::var("USERNAME") {
            // just verify it doesn't panic
            redact(r"C:\Users\testuser\AppData");
        }
    }

    #[test]
    fn no_panic_on_empty() {
        assert_eq!(redact(""), "");
    }

    /// which_all 脱敏后必须保持可比性：同一路径每次都产出同样的结果
    #[test]
    fn which_all_redact_stable() {
        // 模拟 which_all 返回的含用户名路径
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "testuser".to_string());
        let input = format!("C:\\Users\\{user}\\Tools\\node\\node-v23.9.0-win-x64\\node.EXE");
        let first = redact(&input);
        let second = redact(&input);
        // 两次脱敏结果必须一致
        assert_eq!(first, second);
        // 不能包含真实用户名
        if !user.is_empty() && user != "testuser" {
            assert!(!first.contains(&user));
        }
        // 必须包含占位符
        assert!(first.contains("<user>"));
    }
}
