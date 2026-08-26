// 脱敏——默认开启。规格来源：docs/10-架构与数据模型.md §8

use std::env;

/// 脱敏：用户名/机器名/密钥样式串 → 占位符
pub fn redact(s: &str) -> String {
    let mut result = s.to_string();

    // 1. 用户名路径 C:\Users\XXX → C:\Users\<user>
    // ⚠️ 候选用户名：优先 USERPROFILE 的目录名（真实用户目录），
    //    git-bash 里 USERNAME 可能不同（如 USERNAME=sen 但目录是 <user>）。
    let profile_dir = env::var("USERPROFILE")
        .ok()
        .filter(|h| !h.is_empty())
        .and_then(|h| {
            h.trim_end_matches(['\\', '/'])
                .rsplit(['\\', '/'])
                .next()
                .filter(|n| !n.is_empty())
                .map(|n| n.to_string())
        });
    let username = env::var("USERNAME").ok().filter(|u| !u.is_empty());

    let mut user_candidates: Vec<String> = Vec::new();
    if let Some(d) = &profile_dir {
        user_candidates.push(d.clone());
    }
    if let Some(u) = &username {
        if !user_candidates.iter().any(|c| c.eq_ignore_ascii_case(u)) {
            user_candidates.push(u.clone());
        }
    }

    for user in &user_candidates {
        let pattern = format!("\\Users\\{user}");
        let replacement = "\\Users\\<user>";
        result = replace_case_insensitive(&result, &pattern, replacement);

        // 1b. 正斜杠版本：git 等工具输出 C:/Users/XXX（Windows 上分隔符不统一）
        let pattern_fwd = format!("/Users/{user}");
        let replacement_fwd = "/Users/<user>";
        result = replace_case_insensitive(&result, &pattern_fwd, replacement_fwd);
    }

    // 2. HOME / USERPROFILE 路径（反斜杠 + 正斜杠两个形态）
    if let Ok(home) = env::var("USERPROFILE") {
        if !home.is_empty() {
            result = replace_case_insensitive(&result, &home, "<user>");
            let home_fwd = home.replace('\\', "/");
            result = replace_case_insensitive(&result, &home_fwd, "<user>");
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

    // 4.5 邮箱：git config 的 user.email 等（user@example.com → <redacted>）
    result = redact_emails(&result);

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

/// 邮箱 → <redacted>。git config --list --show-origin 会输出 user.email，
/// 报告是要上传的，邮箱也属于隐私。
/// 启发式：local@domain，local 用 [\w.%+-]，domain 用 [\w.-] 且必须含字母和至少一个点。
/// 不命中：纯 IP 域名（127.0.0.1）、URL 认证样式（user:pass@host —— 那是代理规则的事）。
fn redact_emails(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut spans: Vec<(usize, usize)> = Vec::new(); // [start, end) char 区间，左闭右开

    let mut i = 0;
    while i < n {
        if chars[i] == '@' {
            // 域名：@ 后跟 [\w.-]+，至少一个点 + 至少一个字母
            let mut j = i + 1;
            let mut has_dot = false;
            let mut has_letter = false;
            while j < n && (chars[j].is_ascii_alphanumeric() || chars[j] == '.' || chars[j] == '-')
            {
                if chars[j] == '.' {
                    has_dot = true;
                }
                if chars[j].is_ascii_alphabetic() {
                    has_letter = true;
                }
                j += 1;
            }
            // local part：@ 前跟 [\w.%+-]
            let mut k = i;
            while k > 0 {
                let prev = chars[k - 1];
                if prev.is_ascii_alphanumeric()
                    || prev == '.'
                    || prev == '%'
                    || prev == '+'
                    || prev == '-'
                {
                    k -= 1;
                } else {
                    break;
                }
            }
            let local_start = k;
            let local_len = i - local_start;
            let local_ok = local_len >= 1 && !matches!(chars[i - 1], '.' | '%' | '+' | '-');
            let domain_len = j - i - 1;

            if local_ok && has_dot && has_letter && domain_len >= 3 {
                // 防误伤 URL 认证：local part 紧邻 ':'（user:pass@）或 '//'（scheme://）
                let before = if local_start > 0 {
                    chars[local_start - 1]
                } else {
                    '\0'
                };
                let before_scheme = local_start >= 2
                    && chars[local_start - 2] == '/'
                    && chars[local_start - 1] == '/';
                if before != ':' && !before_scheme {
                    spans.push((local_start, j));
                    i = j;
                    continue;
                }
            }
        }
        i += 1;
    }

    // 按区间拼接（区间不重叠）
    let mut out = String::with_capacity(s.len());
    let mut last = 0;
    for (a, b) in spans {
        if a < last {
            continue; // 已被前一个区间覆盖
        }
        out.extend(chars[last..a].iter());
        out.push_str("<redacted>");
        last = b;
    }
    out.extend(chars[last..].iter());
    out
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
                // 无认证信息：scheme 已在上面 push 过，只前进不重复
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

    /// 无认证的 http(s) URL 不能重复 scheme（回归：redact_proxy_auth 曾把
    /// "http://127.0.0.1:7897" 变成 "http://http://127.0.0.1:7897"）
    #[test]
    fn proxy_url_no_dup_scheme() {
        let input = "HTTP_PROXY=http://127.0.0.1:7897";
        let out = redact(input);
        assert_eq!(out, "HTTP_PROXY=http://127.0.0.1:7897");
        // 不能出现重复 scheme
        assert!(!out.contains("http://http://"));
    }

    /// 带账号密码的代理 URL 必须脱敏
    #[test]
    fn proxy_url_with_auth_redacted() {
        let input = "http://user:pass123@127.0.0.1:7897";
        let out = redact(input);
        assert!(out.contains("<redacted>@"), "got: {out}");
        assert!(!out.contains("pass123"), "got: {out}");
    }

    /// git config 的 user.email 必须脱敏
    #[test]
    fn git_email_redacted() {
        let input = "file:C:/Users/me/.gitconfig\tuser.email=user@example.com";
        let out = redact(input);
        assert!(!out.contains("38004547"), "got: {out}");
        assert!(out.contains("<redacted>"), "got: {out}");
    }

    /// 纯 IP 代理 URL 不能被邮箱规则误伤
    #[test]
    fn proxy_url_ip_not_email() {
        let input = "http.proxy=http://127.0.0.1:7897";
        let out = redact(input);
        // 127.0.0.1 有多个点，但 @ 前面没有 local part → 不是邮箱
        assert_eq!(out, "http.proxy=http://127.0.0.1:7897");
    }

    /// git 输出用正斜杠路径（C:/Users/XXX），也必须脱敏
    #[test]
    fn forward_slash_user_path_redacted() {
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "testuser".to_string());
        let input = format!("safe.directory=C:/Users/{user}/proj");
        let out = redact(&input);
        assert!(out.contains("C:/Users/<user>/proj"), "got: {out}");
        if user != "testuser" {
            assert!(!out.contains(&user), "got: {out}");
        }
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
