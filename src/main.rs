//! hypo CLI 入口。
//!
//! 使用 clap derive 定义全部子命令与全局参数，
//! 通过 [`HypoError::exit_code`] 映射 SPEC 5.2 退出码。

#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

/// hypo (High-trust Repository Operator) — 去中心化通用包管理器
#[derive(Parser)]
#[command(name = "hypo", version, about, long_about = None)]
struct Cli {
    #[arg(short, long, global = true)]
    verbose: bool,

    #[arg(short, long, global = true)]
    quiet: bool,

    #[arg(long, global = true)]
    no_color: bool,

    #[arg(long, global = true)]
    config: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 初始化本地配置与目录结构
    Init,

    /// 安装包 @owner/pkg[@<version>]
    Install {
        /// 包名
        package: String,
        /// 强制执行（跳过降级/冻结保护）
        #[arg(short, long)]
        force: bool,
        /// 从自定义 URL 安装
        #[arg(long)]
        from_url: Option<String>,
    },

    /// 卸载包
    Uninstall { package: String },

    /// 列出已安装包
    List,

    /// 查看包详情
    Info { package: String },

    /// 管理 registry
    Registry {
        #[command(subcommand)]
        action: RegistryCmd,
    },

    /// 管理配置
    Config {
        #[command(subcommand)]
        action: ConfigCmd,
    },
}

#[derive(Subcommand)]
enum RegistryCmd {
    /// 添加自定义 registry
    Add { name: String, url: String },
    /// 移除 registry
    Remove { name: String },
    /// 列出已配置 registry
    List,
    /// 导出注册表
    Export { file: String },
}

#[derive(Subcommand)]
enum ConfigCmd {
    /// 获取配置项
    Get { key: String },
    /// 设置配置项
    Set { key: String, value: String },
}

#[tokio::main]
async fn main() {
    let filter = EnvFilter::from_default_env().add_directive(
        "hypo=info"
            .parse()
            .expect("内置 tracing 指令 'hypo=info' 解析失败，这不应发生"),
    );
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Init => hypo::commands::init::run().await,
        Commands::Install {
            package,
            force,
            from_url,
        } => hypo::commands::install::run(&package, force, from_url.as_deref()).await,
        Commands::Uninstall { package } => hypo::commands::uninstall::run(&package).await,
        Commands::List => hypo::commands::list::run().await,
        Commands::Info { package } => hypo::commands::info::run(&package).await,
        Commands::Registry { action } => match action {
            RegistryCmd::Add { name, url } => hypo::commands::registry::add(&name, &url).await,
            RegistryCmd::Remove { name } => hypo::commands::registry::remove(&name).await,
            RegistryCmd::List => hypo::commands::registry::list().await,
            RegistryCmd::Export { file } => hypo::commands::registry::export(&file).await,
        },
        Commands::Config { action } => match action {
            ConfigCmd::Get { key } => hypo::commands::config_cmd::get(&key).await,
            ConfigCmd::Set { key, value } => hypo::commands::config_cmd::set(&key, &value).await,
        },
    };

    if let Err(e) = result {
        eprintln!("错误: {e}");
        std::process::exit(e.exit_code());
    }
}
