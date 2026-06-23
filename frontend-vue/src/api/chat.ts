import type { ModelEntry } from '@/types'

export async function listModels(accessKey: string): Promise<ModelEntry[]> {
  const res = await fetch('/v1/models', {
    headers: { 'Authorization': `Bearer ${accessKey}` },
  })
  if (!res.ok) {
    const err = await res.json().catch(() => ({}))
    throw new Error(err.error?.message || `HTTP ${res.status}`)
  }
  const data = await res.json()
  return data.data || []
}

export async function fetchChatStream(
  url: string,
  body: string,
  accessKey: string,
  signal: AbortSignal,
): Promise<Response> {
  const res = await fetch(url, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${accessKey}`,
    },
    body,
    signal,
  })
  if (!res.ok) {
    let errMsg = `HTTP ${res.status}`
    try {
      const d = await res.json()
      errMsg = d.error?.message || errMsg
    } catch { /* ignore */ }
    throw new Error(errMsg)
  }
  return res
}
