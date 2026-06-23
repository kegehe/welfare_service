const API_BASE = ''

export class ApiError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'ApiError'
  }
}

export async function apiFetch<T>(path: string, opts: RequestInit = {}): Promise<T> {
  const headers: Record<string, string> = {}
  if (opts.headers) {
    const h = new Headers(opts.headers as HeadersInit)
    h.forEach((v, k) => { headers[k] = v })
  }
  if (opts.body) headers['Content-Type'] = 'application/json'

  const res = await fetch(API_BASE + path, { ...opts, headers })
  if (!res.ok) {
    const err = await res.json().catch(() => ({}))
    throw new ApiError(err.error?.message || `HTTP ${res.status}`)
  }
  return res.json()
}
