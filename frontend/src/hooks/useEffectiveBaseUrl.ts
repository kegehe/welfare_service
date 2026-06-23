import { useMemo } from 'react'
import type { PoolKey } from '@/types'

export function useEffectiveBaseUrl(poolKey: PoolKey | null, type: 'claude' | 'codex'): string {
  return useMemo(() => {
    if (!poolKey) return ''

    if (type === 'claude') {
      return poolKey.claude_url || poolKey.openai_url || ''
    } else {
      return poolKey.openai_url || ''
    }
  }, [poolKey, type])
}
