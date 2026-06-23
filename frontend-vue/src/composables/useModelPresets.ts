import { ref } from 'vue'
import { getModelPresets } from '@/api/admin'

// 模块级单例缓存：只请求一次 API，多个组件共享
const presetsCache = ref<Record<string, string[]> | null>(null)
let fetchPromise: Promise<void> | null = null

export function useModelPresets() {
  /** 触发加载预设数据（已加载则跳过，进行中的请求共享同一 Promise） */
  async function ensurePresetsLoaded() {
    if (presetsCache.value !== null) return
    if (fetchPromise) {
      await fetchPromise
      return
    }
    fetchPromise = getModelPresets()
      .then(data => {
        presetsCache.value = data.presets
      })
      .catch(() => {
        // 加载失败时回退为空，el-select 的 allow-create 仍然可用
        presetsCache.value = {}
      })
      .finally(() => {
        fetchPromise = null
      })
    await fetchPromise
  }

  /** 获取指定平台的预设模型列表 */
  function getPresetModelsForPlatform(platform: string): string[] {
    if (!presetsCache.value) return []
    return presetsCache.value[platform] || []
  }

  return {
    presetsCache,
    ensurePresetsLoaded,
    getPresetModelsForPlatform,
  }
}
