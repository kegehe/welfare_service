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
  openai?: { success: boolean; latency_ms: number; status?: number; error?: string } | null
  claude?: { success: boolean; latency_ms: number; status?: number; error?: string } | null
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

// 模型列表条目
export interface ModelEntry {
  id: string
  object: string
  created: number
  owned_by: string
}

// 对话协议类型
export type ChatProtocol = 'openai' | 'claude'

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
