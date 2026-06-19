import { apiFetch } from './index'
import type {
  PoolKey, KeyStatus, AccessKey, HealthInfo,
  TestKeyResult, AddPoolKeyInput, UpdatePoolKeyInput,
  CreateAccessKeyInput, UpdateAccessKeyInput, CreateAccessKeyResponse,
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
