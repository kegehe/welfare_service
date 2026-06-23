// 号池 Key（GET /admin/keys 返回）
export interface PoolKey {
  id: number
  platform: string
  name: string
  api_key: string
  key_prefix: string
  openai_url: string
  claude_url: string
  models: string[]
  tpm_limit: number
  rpm_limit: number
  status: 'active' | 'disabled' | 'unhealthy' | 'expired'
  source: string | null
  note: string | null
  created_at: string | null
}

// 熔断器状态（GET /admin/keys/status 返回）
export interface KeyStatus {
  key_id: number
  circuit_state: 'closed' | 'open' | 'half_open'
  failure_count: number
  tpm_remaining: number
  rpm_remaining: number
  success_rate: number // 0.0 - 1.0
}

// 访问 Key（GET /admin/access-keys 返回）
export interface AccessKey {
  id: number
  key: string
  name: string
  status: 'active' | 'disabled'
  rpm_limit: number
  tpm_limit: number
  expires_at: string | null
  last_used_at: string | null
  total_requests: number
  total_prompt_tokens: number
  total_completion_tokens: number
  created_at: string | null
}

// 健康检查（GET /admin/health 返回）
export interface HealthInfo {
  status: string
  active_keys: number
  version: string
  base_url: string
}

// Key 连通性测试结果（POST /admin/keys/{id}/test 返回）
export interface TestKeyResult {
  key_id: number
  platform: string
  available: boolean
  openai?: { success: boolean; reachable: boolean; key_valid: boolean; upstream_error: boolean; latency_ms: number; status?: number; error?: string } | null
  claude?: { success: boolean; reachable: boolean; key_valid: boolean; upstream_error: boolean; latency_ms: number; status?: number; error?: string } | null
}

// 添加号池 Key 请求体
export interface AddPoolKeyInput {
  platform: string
  name: string | null
  api_key: string
  openai_url: string
  claude_url: string
  models: string[]
  tpm_limit: number
  rpm_limit: number
  source: string | null
  note: string | null
}

// 编辑号池 Key 请求体。api_key 为空表示保留原密钥。
export interface UpdatePoolKeyInput {
  platform: string
  name: string | null
  api_key: string | null
  openai_url: string
  claude_url: string
  models: string[]
  tpm_limit: number
  rpm_limit: number
  source: string | null
  note: string | null
}

// 创建访问 Key 请求体
export interface CreateAccessKeyInput {
  name: string | null
  rpm_limit: number
  tpm_limit: number
  expires_at: string | null
}

// 编辑访问 Key 请求体
export interface UpdateAccessKeyInput {
  name: string | null
  rpm_limit: number
  tpm_limit: number
  expires_at: string | null
}

// 创建访问 Key 响应
export interface CreateAccessKeyResponse {
  id: number
  key: string
  message: string
}

// 用量统计 - 全局概览
export interface StatsOverview {
  total_requests: number
  total_prompt_tokens: number
  total_completion_tokens: number
  total_tokens: number
  active_pool_keys: number
  active_access_keys: number
}

// 用量统计 - 号池 Key 统计行
export interface PoolKeyStats {
  key_id: number
  name: string
  platform: string
  total_requests: number
  total_prompt_tokens: number
  total_completion_tokens: number
  success_rate: number
  avg_latency_ms: number
  last_used_at: string | null
}

// 用量统计 - 访问 Key 统计行
export interface AccessKeyStats {
  access_key_id: number
  name: string
  total_requests: number
  total_prompt_tokens: number
  total_completion_tokens: number
  last_used_at: string | null
}

// 用量统计 - 按模型细分
export interface ModelStats {
  model: string
  requests: number
  prompt_tokens: number
  completion_tokens: number
}

// 用量统计 - 小时趋势数据
export interface HourlyStats {
  hour_bucket: number
  model: string
  request_count: number
  prompt_tokens: number
  completion_tokens: number
}

// 用量统计 - 访问 Key 列表响应
export interface AccessKeyStatsResponse {
  total: {
    total_requests: number
    total_prompt_tokens: number
    total_completion_tokens: number
  }
  keys: AccessKeyStats[]
}

// Key 健康评分（GET /admin/keys/health-score 返回）
export interface KeyHealthScore {
  key_id: number
  health_score: number       // 0-100
  score_source: 'realtime' | 'window' | 'nodata'
  status_label: 'normal' | 'light_throttled' | 'heavy_throttled' | 'key_invalid' | 'nodata'
  sample_count: number
}

// 实时活跃密钥（SSE /admin/keys/active-stream 推送）
export interface ActiveKeyEntry {
  request_id: number
  key_id: number
  key_name: string
  key_prefix: string
  platform: string
  model: string
  started_at: number  // Unix 毫秒时间戳
}

// 模型列表条目
export interface ModelEntry {
  id: string
  object: string
  created: number
  owned_by: string
}

// 对话协议类型
export type ChatProtocol = 'openai' | 'claude'

// 模型预设 (GET /admin/models/presets 返回)
export interface ModelPresetsResponse {
  presets: Record<string, string[]>
}

// 对话历史消息
export interface ChatHistoryMsg {
  role: 'user' | 'assistant' | 'system'
  content: string
}

// SSE 流增量
export interface StreamDelta {
  text: string
  type: 'thinking' | 'text'
}

// 聊天显示消息（UI 用）
export interface DisplayMessage {
  id: string
  role: 'user' | 'assistant' | 'error' | 'system'
  content: string
  thinking: string
  isStreaming: boolean
}
