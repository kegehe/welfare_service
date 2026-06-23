import { useState, useEffect, useCallback } from 'react'
import ReactECharts from 'echarts-for-react'
import { Segmented, Spin } from 'antd'
import { getHourlyStats } from '@/api/admin'
import { COLORS } from '@/config/theme'
import type { HourlyStats } from '@/types'

interface Props {
  hours: number
}

/** 固定调色板 */
const PALETTE = [
  COLORS.pool,      // #06b6d4 青
  COLORS.signal,    // #22c55e 绿
  COLORS.fuse,      // #f59e0b 琥珀
  COLORS.fault,     // #ef4444 红
  COLORS.trace,     // #8b5cf6 紫
  COLORS.lime,      // #84cc16 青绿
  COLORS.systemBlue,// #3b82f6 蓝
  COLORS.flowDark,  // #0891b2 深青
  '#ec4899',        // 粉
  '#14b8a6',        // teal
]

export function ModelDonut({ hours }: Props) {
  const [data, setData] = useState<HourlyStats[]>([])
  const [dimension, setDimension] = useState<'pool' | 'access'>('pool')
  const [loading, setLoading] = useState(false)

  const loadData = useCallback(async () => {
    setLoading(true)
    try {
      const res = await getHourlyStats(dimension, undefined, hours)
      setData(res.data || [])
    } catch {
      // ignore
    } finally {
      setLoading(false)
    }
  }, [dimension, hours])

  useEffect(() => {
    loadData()
  }, [loadData])

  // 按模型聚合 Token 总量
  const modelMap = new Map<string, number>()
  for (const row of data) {
    const total = row.prompt_tokens + row.completion_tokens
    modelMap.set(row.model, (modelMap.get(row.model) || 0) + total)
  }

  // 按 Token 降序排序
  const sorted = [...modelMap.entries()].sort((a, b) => b[1] - a[1])

  const formatTokens = (value: number) => {
    if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
    if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`
    return String(value)
  }

  const option = {
    tooltip: {
      trigger: 'item' as const,
      formatter(params: any) {
        return `${params.marker} ${params.name}<br/><b>${formatTokens(params.value)}</b> (${params.percent}%)`
      },
    },
    legend: {
      orient: 'vertical' as const,
      right: 8,
      top: 'center' as const,
      textStyle: { fontSize: 11 },
      itemWidth: 10,
      itemHeight: 10,
      itemGap: 8,
    },
    color: PALETTE,
    series: [
      {
        type: 'pie' as const,
        radius: ['42%', '70%'],
        center: ['38%', '50%'],
        avoidLabelOverlap: true,
        label: { show: false },
        emphasis: {
          label: { show: true, fontSize: 13, fontWeight: 'bold' as const },
        },
        data: sorted.map(([name, value]) => ({ name, value })),
      },
    ],
  }

  return (
    <div>
      <div style={{ display: 'flex', gap: 8, alignItems: 'center', marginBottom: 8 }}>
        <Segmented
          size="small"
          options={[
            { label: '号池', value: 'pool' },
            { label: '访问', value: 'access' },
          ]}
          value={dimension}
          onChange={(v) => setDimension(v as 'pool' | 'access')}
        />
      </div>
      <Spin spinning={loading}>
        {sorted.length === 0 && !loading ? (
          <div style={{ height: 300, display: 'flex', alignItems: 'center', justifyContent: 'center', color: '#94a3b8' }}>
            暂无数据
          </div>
        ) : (
          <ReactECharts
            option={option}
            style={{ height: 300 }}
            opts={{ renderer: 'canvas' }}
          />
        )}
      </Spin>
    </div>
  )
}
