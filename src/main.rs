mod cli;
mod config;
mod crypto;
mod db;
mod error;
mod health;
mod proxy;
mod scheduler;
mod server;
mod state;

use std::path::{Path, PathBuf};

use clap::Parser;
use tracing_subscriber::EnvFilter;

use cli::{Cli, Commands};
use config::Config;
use crypto::KeyStore;
use db::Database;

#[tokio::main]
async fn main() {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let config_path = PathBuf::from(&cli.config);

    match cli.command {
        Commands::Serve => cmd_serve(config_path).await,
        Commands::GenKey => cmd_gen_key(),
        Commands::AddKey {
            platform,
            key,
            name,
            openai_url,
            claude_url,
            models,
            tpm_limit,
            rpm_limit,
            source,
            note,
        } => cmd_add_key(
            &config_path,
            AddKeyArgs {
                platform: &platform,
                key: &key,
                name: name.as_deref(),
                openai_url: openai_url.as_deref(),
                claude_url: claude_url.as_deref(),
                models: &models,
                tpm_limit,
                rpm_limit,
                source: source.as_deref(),
                note: note.as_deref(),
            },
        ),
        Commands::RemoveKey { id } => cmd_remove_key(&config_path, id),
        Commands::ListKeys => cmd_list_keys(&config_path),
    }
}

/// 启动代理服务
async fn cmd_serve(config_path: PathBuf) {
    let config = Config::load_or_default(&config_path).expect("加载配置失败");

    tracing::info!("启动 Welfare Service v{}", env!("CARGO_PKG_VERSION"));
    tracing::info!("监听地址: {}:{}", config.server.host, config.server.port);

    // 初始化加密存储
    let key_store = if config.encryption.master_key.is_empty() {
        tracing::warn!("未配置主密钥，使用临时密钥 (重启后数据无法解密!)");
        let key = KeyStore::generate_key();
        KeyStore::from_base64_key(&key).expect("创建临时密钥失败")
    } else {
        KeyStore::from_base64_key(&config.encryption.master_key)
            .expect("主密钥格式错误，请使用 `welfare-service gen-key` 生成")
    };

    // 初始化数据库
    let db_path = PathBuf::from(&config.database.path);
    let db = Database::open(&db_path).expect("打开数据库失败");

    // 创建应用状态
    let app_state = state::AppState::new(config.clone(), db, key_store);

    // 注册已有的 Key 到令牌桶和熔断器
    if let Ok(keys) = app_state.db.get_active_keys() {
        for key in &keys {
            app_state.register_pool_key(key);
        }
        tracing::info!("已注册 {} 个 Key 到限流调度器", keys.len());
    }

    // 注册已有的访问 Key 到限流器
    if let Ok(access_keys) = app_state.db.get_active_access_keys() {
        for ak in &access_keys {
            let tpm = if ak.tpm_limit > 0 {
                ak.tpm_limit as u64
            } else {
                0
            };
            let rpm = if ak.rpm_limit > 0 {
                ak.rpm_limit as u64
            } else {
                0
            };
            app_state.access_token_bucket.register(ak.id, tpm, rpm);
        }
        tracing::info!("已注册 {} 个访问 Key 到限流器", access_keys.len());
    }

    // 启动健康检查器
    let health_checker = health::checker::HealthChecker::new(app_state.clone());
    health_checker.start_background();

    // 创建并启动 HTTP 服务 (支持优雅关闭)
    let app = server::create_app(app_state);
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            tracing::error!(
                "端口 {} 已被占用，请先停止旧进程 (可运行 ./stop.sh)",
                config.server.port
            );
            std::process::exit(1);
        }
        Err(e) => {
            tracing::error!("绑定监听地址失败: {}", e);
            std::process::exit(1);
        }
    };

    tracing::info!("服务已就绪: http://{}", addr);

    // 优雅关闭
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("HTTP 服务启动失败");

    tracing::info!("服务已关闭");
}

/// 等待关闭信号
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("安装 Ctrl+C 信号处理器失败");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("安装 SIGTERM 信号处理器失败")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("收到 Ctrl+C 信号，开始关闭..."),
        _ = terminate => tracing::info!("收到 SIGTERM 信号，开始关闭..."),
    }
}

/// 生成主密钥
fn cmd_gen_key() {
    let key = KeyStore::generate_key();
    println!("生成的 AES-256-GCM 主密钥 (base64):");
    println!("{}", key);
    println!();
    println!("请将以下内容添加到 config.toml:");
    println!("[encryption]");
    println!("master_key = \"{}\"", key);
}

struct AddKeyArgs<'a> {
    platform: &'a str,
    key: &'a str,
    name: Option<&'a str>,
    openai_url: Option<&'a str>,
    claude_url: Option<&'a str>,
    models: &'a str,
    tpm_limit: i64,
    rpm_limit: i64,
    source: Option<&'a str>,
    note: Option<&'a str>,
}

/// 添加 API Key
fn cmd_add_key(config_path: &Path, args: AddKeyArgs<'_>) {
    let config = Config::load_or_default(config_path).expect("加载配置失败");

    let key_store = if config.encryption.master_key.is_empty() {
        eprintln!("错误: 请先配置主密钥 (config.toml [encryption] master_key)");
        std::process::exit(1);
    } else {
        KeyStore::from_base64_key(&config.encryption.master_key).expect("主密钥格式错误")
    };

    // 验证平台
    if !config::VALID_PLATFORMS.contains(&args.platform) {
        eprintln!(
            "错误: 无效的平台 '{}'，支持的平台: {:?}",
            args.platform,
            config::VALID_PLATFORMS
        );
        std::process::exit(1);
    }

    let db_path = PathBuf::from(&config.database.path);
    let db = Database::open(&db_path).expect("打开数据库失败");

    let model_list: Vec<String> = args
        .models
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if model_list.is_empty() {
        eprintln!("添加失败: models 不能为空");
        std::process::exit(1);
    }
    let openai_url = args.openai_url.unwrap_or("").trim();
    let claude_url = args.claude_url.unwrap_or("").trim();
    if openai_url.is_empty() && claude_url.is_empty() {
        eprintln!("添加失败: --openai-url 和 --claude-url 至少填写一个");
        std::process::exit(1);
    }
    let encrypted = key_store.encrypt(args.key).expect("加密 API Key 失败");

    let input = db::models::AddApiKeyInput {
        platform: args.platform.to_string(),
        name: args
            .name
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        api_key: args.key.to_string(),
        openai_url: openai_url.to_string(),
        claude_url: claude_url.to_string(),
        models: model_list,
        tpm_limit: Some(args.tpm_limit),
        rpm_limit: Some(args.rpm_limit),
        source: args.source.map(|s| s.to_string()),
        note: args.note.map(|s| s.to_string()),
    };

    match db.add_key(&input, &encrypted) {
        Ok(id) => println!("API Key 添加成功，ID: {}", id),
        Err(e) => {
            eprintln!("添加失败: {}", e);
            std::process::exit(1);
        }
    }
}

/// 移除 API Key
fn cmd_remove_key(config_path: &Path, id: i64) {
    let config = Config::load_or_default(config_path).expect("加载配置失败");
    let db_path = PathBuf::from(&config.database.path);
    let db = Database::open(&db_path).expect("打开数据库失败");

    match db.remove_key(id) {
        Ok(true) => println!("API Key {} 已删除", id),
        Ok(false) => {
            eprintln!("错误: Key ID {} 不存在", id);
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("删除失败: {}", e);
            std::process::exit(1);
        }
    }
}

/// 列出所有 API Key
fn cmd_list_keys(config_path: &Path) {
    let config = Config::load_or_default(config_path).expect("加载配置失败");
    let db_path = PathBuf::from(&config.database.path);
    let db = Database::open(&db_path).expect("打开数据库失败");

    let keys = db.get_all_keys().expect("查询 Key 失败");

    if keys.is_empty() {
        println!("暂无 API Key");
        return;
    }

    println!(
        "{:<4} {:<10} {:<16} {:<20} {:<10} {:<8} {:<8} {:<10}",
        "ID", "平台", "名称", "密钥前缀", "状态", "TPM", "RPM", "模型数"
    );
    println!("{}", "-".repeat(80));

    for key in &keys {
        let model_list: Vec<String> = serde_json::from_str(&key.models).unwrap_or_default();

        // 显示加密后的密钥前缀 (用于标识，非实际密钥)
        let key_display = if key.api_key.len() > 12 {
            format!(
                "{}...{}",
                &key.api_key[..6],
                &key.api_key[key.api_key.len() - 4..]
            )
        } else {
            "****".to_string()
        };

        println!(
            "{:<4} {:<10} {:<16} {:<20} {:<10} {:<8} {:<8} {:<10}",
            key.id,
            key.platform,
            if key.name.is_empty() {
                "-".to_string()
            } else {
                key.name.clone()
            },
            key_display,
            key.status,
            if key.tpm_limit > 0 {
                key.tpm_limit.to_string()
            } else {
                "-".to_string()
            },
            if key.rpm_limit > 0 {
                key.rpm_limit.to_string()
            } else {
                "-".to_string()
            },
            model_list.len()
        );
    }

    println!();
    println!("共 {} 个 Key", keys.len());
}
