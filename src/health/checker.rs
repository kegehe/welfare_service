use std::collections::HashSet;
use std::time::Duration;

use crate::state::AppState;

/// 健康检查器
///
/// 两种模式:
/// 1. 主动探测: 定时发送轻量请求检测 Key 是否可用
/// 2. 被动监测: 根据请求日志分析 Key 健康状况
pub struct HealthChecker {
    state: AppState,
}

impl HealthChecker {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    /// 启动定时健康检查任务
    pub fn start_background(self) {
        let interval = self.state.config.health.check_interval_secs;

        tokio::spawn(async move {
            // 延迟第一次检查，避免启动时的无意义探测
            let first_tick = tokio::time::Instant::now() + Duration::from_secs(interval);
            let mut timer = tokio::time::interval_at(first_tick, Duration::from_secs(interval));
            loop {
                timer.tick().await;
                self.run_checks().await;
            }
        });

        tracing::info!("健康检查器已启动，间隔 {} 秒", interval);
    }

    /// 执行一轮健康检查
    async fn run_checks(&self) {
        let keys = match self.state.db.get_active_keys() {
            Ok(keys) => keys,
            Err(e) => {
                tracing::error!("获取活跃 Key 失败: {}", e);
                return;
            }
        };

        tracing::info!("开始健康检查，共 {} 个活跃 Key", keys.len());
        let mut disabled_this_round = HashSet::new();

        for key in &keys {
            // 被动监测: 检查最近请求的成功率
            match self.state.db.get_key_health_stats(key.id, 20) {
                Ok(stats) => {
                    if stats.total >= self.state.config.health.passive_failure_threshold
                        && stats.success_rate
                            < self.state.config.health.passive_error_rate_threshold
                    {
                        tracing::warn!(
                            "Key {} (平台 {}) 错误率过高: {:.1}% (样本 {}), 自动异常下线",
                            key.id,
                            key.platform,
                            (1.0 - stats.success_rate) * 100.0,
                            stats.total
                        );
                        self.disable_key(key.id);
                        disabled_this_round.insert(key.id);
                        continue;
                    }
                }
                Err(e) => {
                    tracing::error!("获取 Key {} 成功率失败: {}", key.id, e);
                }
            }

            // 被动监测: 检查连续失败次数
            match self.state.db.get_key_consecutive_failures(key.id) {
                Ok(failures) => {
                    if failures >= self.state.config.health.passive_failure_threshold {
                        tracing::warn!(
                            "Key {} (平台 {}) 连续失败 {} 次, 自动异常下线",
                            key.id,
                            key.platform,
                            failures
                        );
                        self.disable_key(key.id);
                        disabled_this_round.insert(key.id);
                        continue;
                    }
                }
                Err(e) => {
                    tracing::error!("获取 Key {} 连续失败次数失败: {}", key.id, e);
                }
            }

            let openai_ok = if key.openai_url.trim().is_empty() {
                None
            } else {
                Some(self.probe_key(key, true).await.is_ok())
            };
            let claude_ok = if key.claude_url.trim().is_empty() {
                None
            } else {
                Some(self.probe_key(key, false).await.is_ok())
            };

            if openai_ok != Some(true) && claude_ok != Some(true) {
                // 所有已配置端点都失败，禁用 Key
                tracing::warn!(
                    "Key {} (平台 {}) 主动探测所有已配置端点均失败，自动异常下线",
                    key.id,
                    key.platform
                );
                self.disable_key(key.id);
                disabled_this_round.insert(key.id);
            } else if openai_ok == Some(false) {
                tracing::warn!("Key {} (平台 {}) OpenAI 端点探测失败", key.id, key.platform);
            } else if claude_ok == Some(false) {
                tracing::warn!("Key {} (平台 {}) Claude 端点探测失败", key.id, key.platform);
            }
        }

        // 尝试重新启用之前被禁用的 Key
        self.try_reenable_keys(&disabled_this_round).await;

        // 清理过期日志
        if let Err(e) = self.state.db.cleanup_old_logs(7) {
            tracing::error!("清理过期日志失败: {}", e);
        }

        tracing::info!("健康检查完成");
    }

    /// 将 Key 标记为健康异常并从运行时调度器移除。
    fn disable_key(&self, key_id: i64) {
        if let Err(e) = self.state.db.update_key_status(key_id, "unhealthy") {
            tracing::error!("标记 Key {} 异常失败: {}", key_id, e);
            return;
        }
        self.state.set_pool_key_disabled(key_id);
    }

    /// 尝试重新启用被禁用的 Key
    async fn try_reenable_keys(&self, disabled_this_round: &HashSet<i64>) {
        let all_keys = match self.state.db.get_all_keys() {
            Ok(keys) => keys,
            Err(_) => return,
        };

        for key in all_keys.iter().filter(|k| k.status == "unhealthy") {
            if disabled_this_round.contains(&key.id) {
                continue;
            }

            let openai_ok = if key.openai_url.trim().is_empty() {
                None
            } else {
                Some(self.probe_key(key, true).await.is_ok())
            };
            let claude_ok = if key.claude_url.trim().is_empty() {
                None
            } else {
                Some(self.probe_key(key, false).await.is_ok())
            };

            if openai_ok == Some(true) || claude_ok == Some(true) {
                tracing::info!(
                    "Key {} (平台 {}) 已恢复 (openai={}, claude={}), 重新启用",
                    key.id,
                    key.platform,
                    openai_ok.unwrap_or(false),
                    claude_ok.unwrap_or(false)
                );
                if let Err(e) = self.state.db.update_key_status(key.id, "active") {
                    tracing::error!("重新启用 Key {} 失败: {}", key.id, e);
                    continue;
                }
                self.state.set_pool_key_active(key);
            }
        }
    }

    /// 主动探测: 发送轻量请求测试 Key
    /// is_openai=true 探测 OpenAI 端点，false 探测 Claude 端点
    async fn probe_key(
        &self,
        key: &crate::db::models::ApiKeyRecord,
        is_openai: bool,
    ) -> Result<(), String> {
        let decrypted_key = self
            .state
            .key_store
            .decrypt(&key.api_key)
            .map_err(|e| format!("解密失败: {}", e))?;

        let base_url = if is_openai {
            key.openai_url.trim()
        } else {
            key.claude_url.trim()
        };
        if base_url.is_empty() {
            return Err("端点未配置".to_string());
        }

        let (probe_url, auth_header) = if is_openai {
            (
                format!("{}/models", base_url.trim_end_matches('/')),
                (
                    "Authorization".to_string(),
                    format!("Bearer {}", decrypted_key),
                ),
            )
        } else {
            // Claude 端点使用 x-api-key 认证
            (
                format!("{}/messages", base_url.trim_end_matches('/')),
                ("x-api-key".to_string(), decrypted_key),
            )
        };

        let mut req = self
            .state
            .http_client
            .get(&probe_url)
            .header(&auth_header.0, &auth_header.1)
            .timeout(Duration::from_secs(
                self.state.config.health.probe_timeout_secs,
            ));

        if !is_openai {
            req = req.header("anthropic-version", "2023-06-01");
        }

        let response = req.send().await.map_err(|e| format!("请求失败: {}", e))?;

        let status = response.status().as_u16();

        if response.status().is_success() || matches!(status, 400 | 404 | 405 | 422 | 429) {
            return Ok(());
        }

        Err(format!("HTTP {}", response.status()))
    }
}
