import { apiFetch } from './index'
import type {
  PoolKey, KeyStatus, AccessKey, HealthInfo,
  TestKeyResult, AddPoolKeyInput, UpdatePoolKeyInput,
  CreateAccessKeyInput, UpdateAccessKeyInput, CreateAccessKeyResponse,
  StatsOverview, PoolKeyStats, AccessKeyStatsResponse, HourlyStats,
  PoolKeyStatsDetail, AccessKeyStatsDetail,
  KeyHealthScore, ModelPresetsResponse,
} from '@/types'

// 号池 Key
export function listPoolKeys() {
  return apiFetch<{ keys: PoolKey[] }>('/admin/keys')
}

export function getKeyStatuses() {
  return apiFetch<{ statuses: KeyStatus[] }>('/admin/keys/status')
}

export function addPoolKey(input: AddPoolKeyInput) {
  return apiFetch<{ id: number; message: string }>('/admin/keys', {
    method: 'POST',
    body: JSON.stringify(input),
  })
}

export function updatePoolKey(id: number, input: UpdatePoolKeyInput) {
  return apiFetch<{ id: number; message: string }>(`/admin/keys/${id}`, {
    method: 'PUT',
    body: JSON.stringify(input),
  })
}

export function deletePoolKey(id: number) {
  return apiFetch<{ message: string }>(`/admin/keys/${id}`, { method: 'DELETE' })
}

export function togglePoolKey(id: number) {
  return apiFetch<{ id: number; status: string; message: string }>(`/admin/keys/${id}/toggle`, { method: 'POST' })
}

export function testPoolKey(id: number) {
  return apiFetch<TestKeyResult>(`/admin/keys/${id}/test`, { method: 'POST' })
}

// 访问 Key
export function listAccessKeys() {
  return apiFetch<{ keys: AccessKey[] }>('/admin/access-keys')
}

export function createAccessKey(input: CreateAccessKeyInput) {
  return apiFetch<CreateAccessKeyResponse>('/admin/access-keys', {
    method: 'POST',
    body: JSON.stringify(input),
  })
}

export function updateAccessKey(id: number, input: UpdateAccessKeyInput) {
  return apiFetch<{ id: number; message: string }>(`/admin/access-keys/${id}`, {
    method: 'PUT',
    body: JSON.stringify(input),
  })
}

export function deleteAccessKey(id: number) {
  return apiFetch<{ message: string }>(`/admin/access-keys/${id}`, { method: 'DELETE' })
}

export function toggleAccessKey(id: number) {
  return apiFetch<{ id: number; status: string; message: string }>(`/admin/access-keys/${id}/toggle`, { method: 'POST' })
}

// 健康检查
export function getHealth() {
  return apiFetch<HealthInfo>('/admin/health')
}

// 用量统计
export function getStatsOverview(hours = 24) {
  return apiFetch<StatsOverview>(`/admin/stats/overview?hours=${hours}`)
}

export function getPoolKeyStats(hours = 24) {
  return apiFetch<{ keys: PoolKeyStats[] }>(`/admin/stats/pool-keys?hours=${hours}`)
}

export function getAccessKeyStats(hours = 24) {
  return apiFetch<AccessKeyStatsResponse>(`/admin/stats/access-keys?hours=${hours}`)
}

export function getHourlyStats(dimension: 'pool' | 'access', keyId?: number, hours = 24) {
  const params = new URLSearchParams({ dimension, hours: String(hours) })
  if (keyId != null) params.set('key_id', String(keyId))
  return apiFetch<{ data: HourlyStats[] }>(`/admin/stats/hourly?${params}`)
}

export function getPoolKeyStatsDetail(id: number, hours = 24) {
  return apiFetch<PoolKeyStatsDetail>(`/admin/stats/pool-keys/${id}?hours=${hours}`)
}

export function getAccessKeyStatsDetail(id: number, hours = 24) {
  return apiFetch<AccessKeyStatsDetail>(`/admin/stats/access-keys/${id}?hours=${hours}`)
}

// Key 健康评分
export function getKeysHealthScore() {
  return apiFetch<KeyHealthScore[]>('/admin/keys/health-score')
}

// 模型预设
export function getModelPresets() {
  return apiFetch<ModelPresetsResponse>('/admin/models/presets')
}
