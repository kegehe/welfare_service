import { create } from 'zustand'
import { message } from 'antd'
import type { PoolKey, KeyStatus, AccessKey, HealthInfo, KeyHealthScore } from '@/types'
import { listPoolKeys, getKeyStatuses, listAccessKeys, getHealth, getKeysHealthScore } from '@/api/admin'

interface AppState {
  poolKeys: PoolKey[]
  keyStatuses: KeyStatus[]
  healthScores: KeyHealthScore[]
  accessKeys: AccessKey[]
  healthInfo: HealthInfo | null
  healthScoreError: string | null
  loading: boolean
  activeTab: string

  setActiveTab: (tab: string) => void
  loadData: () => Promise<void>
}

export const useAppStore = create<AppState>((set) => ({
  poolKeys: [],
  keyStatuses: [],
  healthScores: [],
  accessKeys: [],
  healthInfo: null,
  healthScoreError: null,
  loading: false,
  activeTab: 'overview',

  setActiveTab: (tab) => set({ activeTab: tab }),

  loadData: async () => {
    set({ loading: true })
    try {
      const [poolData, healthData, poolStatus, accessData] = await Promise.all([
        listPoolKeys(),
        getHealth(),
        getKeyStatuses(),
        listAccessKeys(),
      ])
      set({
        poolKeys: poolData.keys || [],
        keyStatuses: poolStatus.statuses || [],
        accessKeys: accessData.keys || [],
        healthInfo: healthData,
      })

      // 健康评分独立加载，失败时降级
      try {
        const healthScoreData = await getKeysHealthScore()
        set({
          healthScores: Array.isArray(healthScoreData) ? healthScoreData : [],
          healthScoreError: null,
        })
      } catch (e: any) {
        set({
          healthScores: [],
          healthScoreError: e?.message || '健康评分加载失败',
        })
      }
    } catch (e: any) {
      message.error('加载失败: ' + (e.message || '未知错误'))
    } finally {
      set({ loading: false })
    }
  },
}))
