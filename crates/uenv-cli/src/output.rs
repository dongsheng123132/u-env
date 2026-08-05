// CLI 输出层。规格来源：docs/10-架构与数据模型.md §7
//
// 核心规则：
//   stdout = 结果数据（机器读）
//   stderr = 日志/进度/调试（人读）

use serde::Serialize;
use std::io::{IsTerminal, Write};

/// JSON 顶层包装
#[derive(Serialize)]
pub struct JsonOutput<T: Serialize> {
    pub ok: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub stats: Option<OutputStats>,
}

#[derive(Serialize)]
pub struct OutputStats {
    pub elapsed_ms: u64,
    pub count: usize,
}

/// 输出上下文：控制 TTY检测、JSON 模式、安静模式
#[allow(dead_code)]
pub struct Output {
    pub json_mode: bool,
    pub quiet: bool,
    pub verbose: bool,
}

impl Output {
    pub fn new(json: bool, quiet: bool, verbose: bool) -> Self {
        Self {
            json_mode: json,
            quiet,
            verbose,
        }
    }

    /// 写入 stderr（人读日志/进度），JSON 模式下若非 verbose 则抑制
    pub fn log(&self, msg: &str) {
        if self.json_mode && !self.verbose {
            return;
        }
        let mut stderr = std::io::stderr();
        let _ = writeln!(stderr, "{msg}");
    }

    /// 写入 stdout 纯文本（仅非 JSON 模式）
    pub fn text(&self, msg: &str) {
        if self.json_mode {
            return;
        }
        let mut stdout = std::io::stdout();
        let _ = writeln!(stdout, "{msg}");
    }

    /// 输出 JSON 到 stdout
    pub fn json<T: Serialize>(
        &self,
        ok: bool,
        data: Option<T>,
        error: Option<String>,
        stats: Option<OutputStats>,
    ) {
        let output = JsonOutput {
            ok,
            data,
            error,
            stats,
        };
        let json_str = serde_json::to_string_pretty(&output)
            .unwrap_or_else(|e| format!(r#"{{"ok":false,"error":"JSON serialize failed: {e}"}}"#));
        let mut stdout = std::io::stdout();
        let _ = writeln!(stdout, "{json_str}");
    }

    /// 标准文本输出：非 JSON 时用 text，JSON 时用 json
    #[allow(dead_code)]
    pub fn result<T: Serialize>(
        &self,
        ok: bool,
        data: Option<T>,
        error: Option<String>,
        stats: Option<OutputStats>,
    ) {
        if self.json_mode {
            self.json(ok, data, error, stats);
        } else if let Some(ref err) = error {
            self.text(&format!("Error: {err}"));
        } else if let Some(ref d) = data {
            let s = serde_json::to_string_pretty(d)
                .unwrap_or_else(|_| String::from("(serialize error)"));
            self.text(&s);
        }
    }
}

/// stdout 是否为 TTY（人在敲）
#[allow(dead_code)]
pub fn is_tty() -> bool {
    std::io::stdout().is_terminal()
}
