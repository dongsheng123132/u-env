use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use uenv_core::EvidenceKind;
use winreg::RegKey;

use crate::redact;
use crate::util::{decode_process_output, find_all_in_path};

/// 命令执行结果
#[derive(Debug, Clone)]
pub struct CommandOutcome {
    /// 程序是否存在并启动
    pub ran: bool,
    pub exit_code: Option<i32>,
    /// 已解码 + 已脱敏
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub elapsed_ms: u64,
}

/// 注册表读取结果
#[derive(Debug, Clone)]
pub struct RegValue {
    pub value: String,
    pub kind: String,
}

/// ScanContext 是 detector 访问系统信息的唯一入口。
pub struct ScanContext {
    pub project_root: Option<PathBuf>,
    pub redact: bool,
    pub timeout: Duration,
}

impl Default for ScanContext {
    fn default() -> Self {
        Self {
            project_root: None,
            redact: true,
            timeout: Duration::from_secs(10),
        }
    }
}

impl ScanContext {
    /// 唯一允许的外部命令调用入口。必须带超时、捕获退出码、
    /// 处理 GBK/UTF-8/UTF-16 输出。
    pub fn run(&self, program: &str, args: &[&str]) -> CommandOutcome {
        self.run_with_timeout(program, args, self.timeout)
    }

    /// 慢命令专用入口：npm/dotnet/vswhere 等启动慢的，单独放宽到 20s。
    /// 与 run() 共用同一套超时/解码/脱敏逻辑。
    pub fn run_slow(&self, program: &str, args: &[&str]) -> CommandOutcome {
        self.run_with_timeout(program, args, Duration::from_secs(20))
    }

    fn run_with_timeout(
        &self,
        program: &str,
        args: &[&str],
        timeout: Duration,
    ) -> CommandOutcome {
        let start = Instant::now();

        // 直接 spawn；失败（Windows 上 .cmd/.bat 脚本 CreateProcess 不认）时
        // fallback 到 cmd /c —— npm/pnpm/yarn/corepack 都是 .cmd 脚本。
        let mut child = match spawn_direct(program, args) {
            Ok(c) => c,
            Err(first_err) => match spawn_via_cmd(program, args) {
                Ok(c) => c,
                Err(_) => {
                    return CommandOutcome {
                        ran: false,
                        exit_code: None,
                        stdout: String::new(),
                        stderr: format!("failed to spawn {program}: {first_err}"),
                        timed_out: false,
                        elapsed_ms: start.elapsed().as_millis() as u64,
                    };
                }
            },
        };

        // 轮询等待完成或超时（std 没有 wait_timeout，用 try_wait + sleep）
        let poll_interval = Duration::from_millis(100);
        let deadline = start + timeout;

        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    // 正常退出
                    let exit_code = status.code();
                    let elapsed = start.elapsed().as_millis() as u64;

                    let mut stdout = Vec::new();
                    let mut stderr = Vec::new();
                    let _ = child.stdout.take().unwrap().read_to_end(&mut stdout);
                    let _ = child.stderr.take().unwrap().read_to_end(&mut stderr);

                    let (stdout_s, stderr_s) = decode_process_output(&stdout, &stderr);

                    let stdout_s = if self.redact {
                        redact::redact(&stdout_s)
                    } else {
                        stdout_s
                    };
                    let stderr_s = if self.redact {
                        redact::redact(&stderr_s)
                    } else {
                        stderr_s
                    };

                    return CommandOutcome {
                        ran: true,
                        exit_code,
                        stdout: stdout_s,
                        stderr: stderr_s,
                        timed_out: false,
                        elapsed_ms: elapsed,
                    };
                }
                Ok(None) => {
                    // 还在运行
                    if Instant::now() >= deadline {
                        break;
                    }
                    thread::sleep(poll_interval);
                }
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return CommandOutcome {
                        ran: true,
                        exit_code: None,
                        stdout: String::new(),
                        stderr: format!("wait error: {e}"),
                        timed_out: false,
                        elapsed_ms: start.elapsed().as_millis() as u64,
                    };
                }
            }
        }

        // 超时——loop 只在超时时 break 到达这里
        {
            let _ = child.kill();
            let _ = child.wait();
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let _ = child.stdout.take().unwrap().read_to_end(&mut stdout);
            let _ = child.stderr.take().unwrap().read_to_end(&mut stderr);

            let (stdout_s, stderr_s) = decode_process_output(&stdout, &stderr);

            let stdout_s = if self.redact {
                redact::redact(&stdout_s)
            } else {
                stdout_s
            };
            let stderr_s = if self.redact {
                redact::redact(&stderr_s)
            } else {
                stderr_s
            };

            CommandOutcome {
                ran: true,
                exit_code: None,
                stdout: stdout_s,
                stderr: format!(
                    "timed out after {:.0}s\n{}",
                    timeout.as_secs_f64(),
                    stderr_s
                ),
                timed_out: true,
                elapsed_ms: start.elapsed().as_millis() as u64,
            }
        }
    }

    /// 注册表读取入口（winreg 封装，失败返回 None 而不是 panic）
    pub fn reg_read(&self, hive: winreg::HKEY, path: &str, name: &str) -> Option<RegValue> {
        let key = RegKey::predef(hive);
        let subkey = key.open_subkey(path).ok()?;
        let raw: winreg::RegValue = subkey.get_raw_value(name).ok()?;

        let value_str = match raw.vtype {
            winreg::enums::RegType::REG_SZ | winreg::enums::RegType::REG_EXPAND_SZ => {
                // Windows 注册表 REG_SZ 是 UTF-16LE 以 NUL 结尾
                decode_reg_sz(&raw.bytes)
            }
            winreg::enums::RegType::REG_DWORD => {
                if raw.bytes.len() >= 4 {
                    let val = u32::from_le_bytes([
                        raw.bytes[0],
                        raw.bytes[1],
                        raw.bytes[2],
                        raw.bytes[3],
                    ]);
                    format!("{val}")
                } else {
                    String::new()
                }
            }
            _ => String::from_utf8_lossy(&raw.bytes)
                .trim_end_matches('\0')
                .to_string(),
        };

        let value_str = if self.redact {
            redact::redact(&value_str)
        } else {
            value_str
        };

        Some(RegValue {
            value: value_str,
            kind: format!("{:?}", raw.vtype),
        })
    }

    /// PATH 里查同名可执行文件的**全部**命中（不是第一个）——冲突检测靠它。
    /// 返回值已脱敏（用户目录 → `<user>`），可直接进 facts/evidence。
    /// 注意：脱敏后的路径不能用于命令执行——如需执行，用 `ctx.run(exe_name, args)`。
    pub fn which_all(&self, exe: &str) -> Vec<PathBuf> {
        let paths = find_all_in_path(exe);
        if self.redact {
            paths
                .into_iter()
                .map(|p| PathBuf::from(redact::redact(&p.to_string_lossy())))
                .collect()
        } else {
            paths
        }
    }

    /// 脱敏：用户名/机器名/密钥样式串 → 占位符
    pub fn redact(&self, s: &str) -> String {
        redact::redact(s)
    }
}

/// 解码 Windows 注册表 REG_SZ 值（UTF-16LE，以 NUL 结尾）
fn decode_reg_sz(bytes: &[u8]) -> String {
    if bytes.len() < 2 {
        return String::new();
    }
    // 将字节对解释为 UTF-16LE 字符
    let u16s: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let s = String::from_utf16_lossy(&u16s);
    // 去除尾部 NUL
    s.trim_end_matches('\0').to_string()
}

/// 直接 spawn 子进程（stdin/stdout/stderr 全接管）
fn spawn_direct(program: &str, args: &[&str]) -> std::io::Result<std::process::Child> {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

/// 通过 cmd /c 运行：Windows 上 .cmd/.bat 脚本（npm/pnpm/yarn/corepack）
/// CreateProcess 不认，必须交给 cmd.exe。仅作为 spawn 失败的兜底。
fn spawn_via_cmd(program: &str, args: &[&str]) -> std::io::Result<std::process::Child> {
    let mut cmdline = program.to_string();
    for a in args {
        cmdline.push(' ');
        if a.contains(' ') || a.contains('&') || a.contains('|') || a.contains('<') || a.contains('>') {
            cmdline.push('"');
            cmdline.push_str(a);
            cmdline.push('"');
        } else {
            cmdline.push_str(a);
        }
    }
    Command::new("cmd")
        .args(["/c", &cmdline])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

/// 从 CommandOutcome 构建 Evidence
pub fn evidence_from_command(
    kind: EvidenceKind,
    source: &str,
    outcome: &CommandOutcome,
) -> uenv_core::Evidence {
    let excerpt = if outcome.stdout.len() > 2000 {
        &outcome.stdout[..2000]
    } else {
        &outcome.stdout
    };
    uenv_core::Evidence {
        kind,
        source: source.to_string(),
        exit_code: outcome.exit_code,
        excerpt: excerpt.to_string(),
    }
}

/// 从注册表读取构建 Evidence
pub fn evidence_from_registry(
    path: &str,
    name: &str,
    value: &Option<RegValue>,
) -> uenv_core::Evidence {
    let excerpt = match value {
        Some(v) => {
            let s = &v.value;
            if s.len() > 2000 {
                s[..2000].to_string()
            } else {
                s.clone()
            }
        }
        None => "(not found)".to_string(),
    };
    uenv_core::Evidence {
        kind: EvidenceKind::Registry,
        source: format!("{path}\\{name}"),
        exit_code: None,
        excerpt,
    }
}
