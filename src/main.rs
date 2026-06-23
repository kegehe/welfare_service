mod cli;
mod config;
mod crypto;
mod db;
mod error;
mod health;
mod health_score_cache;
mod proxy;
mod scheduler;
mod server;
mod state;
mod usage_cache;

use std::future::IntoFuture;
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::Parser;
use tokio_util::sync::CancellationToken;
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

    // 创建共享的取消令牌，用于协调所有后台任务和 handler 的关闭
    let cancel_token = CancellationToken::new();

    // 创建应用状态
    let app_state = state::AppState::new(config.clone(), db, key_store, cancel_token.clone());

    // 注册已有的 Key 到令牌桶和熔断器
    if let Ok(keys) = app_state.db.get_active_keys() {
        for key in &keys {
            app_state.register_pool_key(key);
        }
        tracing::info!("已注册 {} 个 Key 到限流调度器", keys.len());
    }

    match app_state.db.load_circuit_states() {
        Ok(states) => {
            let count = states.len();
            for item in states {
                app_state.circuit_breaker.restore(
                    item.key_id,
                    item.state,
                    item.failure_count,
                    item.last_failure_at,
                    item.opened_at,
                );
            }
            if count > 0 {
                tracing::info!("已恢复 {} 个熔断器状态", count);
            }
        }
        Err(e) => tracing::warn!("恢复熔断器状态失败 (不影响启动): {}", e),
    }

    match app_state.db.load_rate_limit_cooldowns() {
        Ok(cooldowns) => {
            let count = cooldowns.len();
            for item in cooldowns {
                app_state.rate_limit_cooldown.mark_limited_for(
                    item.key_id,
                    Duration::from_secs(item.remaining_secs),
                );
            }
            if count > 0 {
                tracing::info!("已恢复 {} 个 429 冷却状态", count);
            }
        }
        Err(e) => tracing::warn!("恢复 429 冷却状态失败 (不影响启动): {}", e),
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

    // 加载用量缓存（从数据库恢复当前小时数据）
    if let Err(e) = app_state.usage_cache.load_from_db(&app_state.db) {
        tracing::warn!("加载用量缓存失败 (不影响服务): {}", e);
    }

    // 启动用量缓存刷盘定时任务
    let flush_handle = usage_cache::start_flush_task(
        app_state.usage_cache.clone(),
        app_state.db.clone(),
        cancel_token.clone(),
    );

    // 启动健康检查器
    let health_checker = health::checker::HealthChecker::new(app_state.clone(), cancel_token.clone());
    let health_handle = health_checker.start_background();

    // 创建并启动 HTTP 服务 (支持优雅关闭)
    let app = server::create_app(app_state.clone());
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            tracing::error!(
                "端口 {} 已被占用，请先停止旧进程 (可运行 ./stop.sh)",
                config.server.port
            );
            // 取消后台任务后再退出
            cancel_token.cancel();
            let _ = health_handle.await;
            let _ = flush_handle.await;
            std::process::exit(1);
        }
        Err(e) => {
            tracing::error!("绑定监听地址失败: {}", e);
            cancel_token.cancel();
            let _ = health_handle.await;
            let _ = flush_handle.await;
            std::process::exit(1);
        }
    };

    tracing::info!("服务已就绪: http://{}", addr);

    // 优雅关闭: 先等待关闭信号，再关闭 HTTP 服务
    //
    // 之前的 bug: tokio::time::timeout(30s, serve.with_graceful_shutdown(signal))
    // timeout 从服务启动就开始计时，30 秒后直接 drop server future，导致服务自动退出。
    //
    // 正确做法: 将"等待信号"和"等待连接关闭"分为两阶段:
    // 阶段1: spawn 服务(带 graceful_shutdown)在后台运行，同时等待关闭信号（无超时）
    // 阶段2: 收到信号后触发 graceful_shutdown（通过 CancellationToken），
    //         等待 server 任务完成（30秒超时兜底，防止 SSE 长连接卡住）

    // 创建一个专用于 HTTP 服务的关闭信号
    // 当 cancel_token 被取消时，graceful_shutdown 信号完成，axum 停止接受新连接
    let cancel_for_http = cancel_token.clone();
    let http_shutdown = async move {
        cancel_for_http.cancelled().await;
    };

    // 阶段1: 启动 HTTP 服务（带 graceful_shutdown）
    let server_task = tokio::spawn(
        axum::serve(listener, app).with_graceful_shutdown(http_shutdown).into_future(),
    );

    // 等待关闭信号（Ctrl+C / SIGTERM），无超时限制
    shutdown_signal(cancel_token.clone()).await;

    // 信号已收到，触发 CancellationToken:
    // - HTTP 服务的 graceful_shutdown 信号完成 → 停止接受新连接
    // - SSE 和 proxy 流主动断开
    // - 健康检查器和刷盘任务开始关闭
    cancel_token.cancel();
    tracing::info!("收到关闭信号，开始优雅关闭...");

    // 阶段2: 等待 HTTP 服务关闭（现有连接完成或 30 秒超时强制退出）
    // with_graceful_shutdown 会停止 accept 新连接并等待现有连接完成
    // cancel_token 取消后 SSE/proxy 流会主动断开，所以连接应该很快关闭
    let graceful_result = tokio::time::timeout(Duration::from_secs(30), server_task).await;

    match graceful_result {
        Ok(Ok(Ok(()))) => {
            tracing::info!("HTTP 服务已正常关闭");
        }
        Ok(Ok(Err(e))) => {
            tracing::error!("HTTP 服务错误: {}", e);
        }
        Ok(Err(e)) => {
            tracing::error!("HTTP 服务任务异常: {}", e);
        }
        Err(_) => {
            tracing::warn!("HTTP 服务在 30 秒内未完成关闭，强制退出（可能有残留 SSE 连接）");
        }
    }

    tracing::info!("HTTP 服务已关闭，等待后台任务退出...");

    if let Err(e) = app_state
        .db
        .save_circuit_snapshot(&app_state.circuit_breaker.snapshot())
    {
        tracing::warn!("保存熔断器状态失败: {}", e);
    }
    if let Err(e) = app_state
        .db
        .save_rate_limit_cooldowns(&app_state.rate_limit_cooldown.snapshot_remaining_secs())
    {
        tracing::warn!("保存 429 冷却状态失败: {}", e);
    }

    // 等待后台任务退出，设置超时兜底（防止卡死，第二次 Ctrl+C 可强制退出）
    let shutdown_timeout = Duration::from_secs(10);
    let health_result = tokio::time::timeout(shutdown_timeout, health_handle).await;
    match health_result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::error!("健康检查器退出异常: {}", e),
        Err(_) => tracing::warn!("健康检查器在 {} 秒内未退出，跳过等待", shutdown_timeout.as_secs()),
    }

    let flush_result = tokio::time::timeout(shutdown_timeout, flush_handle).await;
    match flush_result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::error!("刷盘任务退出异常: {}", e),
        Err(_) => {
            tracing::warn!("刷盘任务在 {} 秒内未退出，执行兜底刷盘...", shutdown_timeout.as_secs());
            if let Err(e) = app_state.usage_cache.flush(&app_state.db) {
                tracing::error!("兜底刷盘失败: {}", e);
            } else {
                tracing::info!("兜底刷盘完成");
            }
        }
    }

    tracing::info!("所有后台任务已退出，服务关闭完成");
}

/// 等待关闭信号
///
/// 收到信号后触发 CancellationToken，通知所有后台任务开始关闭。
/// 首次信号后注册第二次 SIGINT 的强制退出 handler，确保用户能通过再次 Ctrl+C 强制终止。
async fn shutdown_signal(cancel: CancellationToken) {
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
        _ = ctrl_c => {
            tracing::info!("收到 Ctrl+C 信号，开始关闭...");
            cancel.cancel();
        }
        _ = terminate => {
            tracing::info!("收到 SIGTERM 信号，开始关闭...");
            cancel.cancel();
        }
    }

    // 首次信号后，注册第二次 SIGINT 的强制退出 handler
    // tokio::signal::ctrl_c() 注册的 handler 会永久拦截 SIGINT，
    // 导致第二次 Ctrl+C 被吞掉而非终止进程。
    // 重新 await ctrl_c 并在到达时直接退出，让用户可以强制终止。
    tokio::spawn(async {
        tokio::signal::ctrl_c()
            .await
            .expect("安装 Ctrl+C 信号处理器失败");
        tracing::warn!("收到第二次 Ctrl+C 信号，强制退出");
        std::process::exit(130); // 128 + SIGINT(2)，与默认 SIGINT 行为一致
    });
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
