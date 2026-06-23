import { useState, useEffect, useRef, useCallback } from 'react'
import { getModelPresets } from '@/api/admin'

// 模块级单例缓存：只请求一次 API，多个组件共享
let presetsCache: Record<string, string[]> | null = null
let fetchPromise: Promise<void> | null = null

async function ensurePresetsLoaded() {
  if (presetsCache !== null) return
  if (fetchPromise) {
    await fetchPromise
    return
  }
  fetchPromise = getModelPresets()
    .then(data => {
      presetsCache = data.presets
    })
    .catch(() => {
      presetsCache = {}
    })
    .finally(() => {
      fetchPromise = null
    })
  await fetchPromise
}

export function useModelPresets() {
  const [presets, setPresets] = useState<Record<string, string[]> | null>(presetsCache)
  const mountedRef = useRef(true)

  useEffect(() => {
    mountedRef.current = true

    if (presetsCache !== null) {
      setPresets(presetsCache)
      return
    }

    ensurePresetsLoaded().then(() => {
      if (mountedRef.current) {
        setPresets(presetsCache)
      }
    })

    return () => {
      mountedRef.current = false
    }
  }, [])

  /** 获取指定平台的预设模型列表 */
  const getPresetModelsForPlatform = useCallback((platform: string): string[] => {
    if (!presets) return []
    return presets[platform] || []
  }, [presets])

  return {
    presets,
    ensurePresetsLoaded,
    getPresetModelsForPlatform,
  }
}
