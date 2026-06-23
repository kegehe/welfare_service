import { useState, useEffect, useCallback } from 'react'
import ReactECharts from 'echarts-for-react'
import { Segmented, Spin } from 'antd'
import { getHourlyStats } from '@/api/admin'
import { COLORS } from '@/config/theme'
import type { HourlyStats } from '@/types'

interface Props {
  hours: number
}

export function UsageChart({ hours }: Props) {
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

  // 按小时聚合
  const hourMap = new Map<number, { prompt: number; completion: number }>()
  for (const row of data) {
    const existing = hourMap.get(row.hour_bucket) || { prompt: 0, completion: 0 }
    existing.prompt += row.prompt_tokens
    existing.completion += row.completion_tokens
    hourMap.set(row.hour_bucket, existing)
  }

  const hoursList = [...hourMap.keys()].sort()
  const promptData = hoursList.map(h => hourMap.get(h)!.prompt)
  const completionData = hoursList.map(h => hourMap.get(h)!.completion)
  const labels = hoursList.map(h => {
    const d = new Date(h * 3600 * 1000)
    return `${String(d.getHours()).padStart(2, '0')}:00`
  })

  const formatTokens = (value: number) => {
    if (value >= 1000000) return `${(value / 1000000).toFixed(1)}M`
    if (value >= 1000) return `${(value / 1000).toFixed(1)}K`
    return String(value)
  }

  const option = {
    tooltip: {
      trigger: 'axis' as const,
      axisPointer: { type: 'shadow' as const },
      formatter(params: any) {
        const items = Array.isArray(params) ? params : [params]
        let html = `<div style="font-weight:600;margin-bottom:4px">${items[0]?.axisValue}</div>`
        for (const item of items) {
          html += `<div>${item.marker} ${item.seriesName}: <b>${formatTokens(item.value)}</b></div>`
        }
        return html
      }
    },
    legend: {
      data: ['输入 Tokens', '输出 Tokens'],
      top: 0,
      textStyle: { fontSize: 11 }
    },
    grid: { left: 50, right: 16, top: 32, bottom: 32 },
    xAxis: {
      type: 'category' as const,
      data: labels,
    },
    yAxis: {
      type: 'value' as const,
      axisLabel: {
        formatter: (v: number) => formatTokens(v)
      }
    },
    series: [
      {
        name: '输入 Tokens',
        type: 'bar' as const,
        stack: 'total',
        data: promptData,
        itemStyle: { color: COLORS.pool },
      },
      {
        name: '输出 Tokens',
        type: 'bar' as const,
        stack: 'total',
        data: completionData,
        itemStyle: { color: COLORS.signal },
      },
    ],
  }

  return (
    <div>
      <div className="chart-toolbar">
        <Segmented
          options={[
            { label: '号池', value: 'pool' },
            { label: '访问', value: 'access' },
          ]}
          value={dimension}
          onChange={(v) => setDimension(v as 'pool' | 'access')}
        />
      </div>
      <Spin spinning={loading}>
        <ReactECharts
          option={option}
          style={{ height: 400 }}
          opts={{ renderer: 'canvas' }}
        />
      </Spin>
    </div>
  )
}
