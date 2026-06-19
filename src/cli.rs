use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "welfare-service")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "API Key 池化共享服务 - Linux.do 社区共享 API Key 代理")]
pub struct Cli {
    /// 配置文件路径
    #[arg(short, long, default_value = "config.toml")]
    pub config: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 启动代理服务
    Serve,

    /// 生成 AES-256-GCM 主密钥
    GenKey,

    /// 添加 API Key
    AddKey {
        /// 平台名称 (xiaomi / iflytek)
        #[arg(short, long)]
        platform: String,

        /// API Key
        #[arg(short, long)]
        key: String,

        /// Key 名称
        #[arg(long)]
        name: Option<String>,

        /// OpenAI 兼容 Base URL
        #[arg(long)]
        openai_url: Option<String>,

        /// Claude 兼容 Base URL
        #[arg(long)]
        claude_url: Option<String>,

        /// 支持的模型列表 (逗号分隔)
        #[arg(short, long)]
        models: String,

        /// TPM 限制
        #[arg(long, default_value = "0")]
        tpm_limit: i64,

        /// RPM 限制
        #[arg(long, default_value = "0")]
        rpm_limit: i64,

        /// 来源说明
        #[arg(long)]
        source: Option<String>,

        /// 备注
        #[arg(long)]
        note: Option<String>,
    },

    /// 移除 API Key
    RemoveKey {
        /// Key ID
        id: i64,
    },

    /// 列出所有 API Key
    ListKeys,
}
