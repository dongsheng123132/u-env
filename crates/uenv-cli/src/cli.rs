use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// u-env — 本境协议 Origin Environment Protocol 参考实现
/// 扫描、理解、比较、修复 Windows 软件开发环境。
#[derive(Parser)]
#[command(name = "uenv", version, about)]
pub struct Cli {
    /// JSON 输出模式
    #[arg(long, global = true)]
    pub json: bool,

    /// 静默模式
    #[arg(short = 'q', long, global = true)]
    pub quiet: bool,

    /// 详细输出
    #[arg(short = 'v', long, global = true)]
    pub verbose: bool,

    /// 不等待交互输入
    #[arg(long, global = true)]
    pub no_input: bool,

    /// 自动确认所有提示
    #[arg(short = 'y', long, global = true)]
    pub yes: bool,

    /// 项目根目录
    #[arg(long, global = true)]
    pub project: Option<PathBuf>,

    /// 禁用脱敏
    #[arg(long, global = true)]
    pub no_redact: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 扫描环境，生成 environment.origin.json
    Scan {
        /// 输出文件路径（默认 stdout）
        #[arg(long)]
        out: Option<PathBuf>,
    },

    /// 诊断环境问题
    Doctor {
        /// 失败阈值：none | warning | error
        #[arg(long, default_value = "error")]
        fail_on: String,

        /// 从已有快照文件诊断（缺省 = 现场 scan）
        #[arg(long)]
        from: Option<PathBuf>,

        /// agent 模式 = --json --quiet --no-input --fail-on error（给 AI 用）
        #[arg(long)]
        agent: bool,
    },

    /// 生成报告（markdown / json）
    Report {
        /// 输出格式：markdown | json
        #[arg(long, default_value = "markdown")]
        format: String,

        /// 输出文件路径
        #[arg(long)]
        out: Option<PathBuf>,
    },

    /// 计算环境指纹
    Fingerprint {
        /// 从已有快照文件计算（缺省 = 现场 scan 后计算）
        #[arg(long)]
        from: Option<PathBuf>,
    },

    /// 比较两个环境快照
    Diff {
        /// 基准快照
        a: PathBuf,
        /// 比较快照
        b: PathBuf,
    },

    /// 输出/写入 agent 发现 stub（给 AI 指路，只读安全）
    Stub {
        /// 输出文件路径（缺省 = 打印到 stdout）
        #[arg(long)]
        out: Option<PathBuf>,
    },
}
