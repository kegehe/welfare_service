import { useState, useEffect, useCallback } from 'react'
import ReactECharts from 'echarts-for-react'
import { Segmented, Select, Spin } from 'antd'
import { getHourlyStats } from '@/api/admin'
import { COLORS } from '@/config/theme'
import type { HourlyStats } from '@/types'

interface Props {
  hours: number
  poolKeys?: { key_id: number; name: string }[]
  accessKeys?: { access_key_id: number; name: string }[]
}

/**
 * 请求量趋势折线图
 * 利用 hourly API 返回的 request_count 字段（原有 UsageChart 忽略此字段）
 */
export function RequestChart({ hours, poolKeys, accessKeys }: Props) {
  const [data, setData] = useState<HourlyStats[]>([])
  const [dimension, setDimension] = useState<'pool' | 'access'>('pool')
  const [keyId, setKeyId] = useState<number | undefined>(undefined)
  const [loading, setLoading] = useState(false)

  const isMultiDay = hours > 48

  const loadData = useCallback(async () => {
    setLoading(true)
    try {
      const res = await getHourlyStats(dimension, keyId, hours)
      setData(res.data || [])
    } catch {
      // ignore
    } finally {
      setLoading(false)
    }
  }, [dimension, keyId, hours])

  useEffect(() => {
    loadData()
  }, [loadData])

  useEffect(() => {
    setKeyId(undefined)
  }, [dimension])

  const currentKeys = dimension === 'pool' ? poolKeys : accessKeys
  const keyIdField = dimension === 'pool' ? 'key_id' : 'access_key_id'

  // 按小时聚合请求数
  const hourMap = new Map<number, number>()
  for (const row of data) {
    hourMap.set(row.hour_bucket, (hourMap.get(row.hour_bucket) || 0) + row.request_count)
  }

  const hoursList = [...hourMap.keys()].sort()
  const requestData = hoursList.map(h => hourMap.get(h)!)
  const labels = hoursList.map(h => {
    const d = new Date(h * 3600 * 1000)
    if (isMultiDay) {
      const month = String(d.getMonth() + 1).padStart(2, '0')
      const day = String(d.getDate()).padStart(2, '0')
      const hr = String(d.getHours()).padStart(2, '0')
      return `${month}-${day} ${hr}:00`
    }
    return `${String(d.getHours()).padStart(2, '0')}:00`
  })

  const axisLabelInterval = isMultiDay
    ? Math.max(0, Math.floor(hoursList.length / 20) - 1)
    : 'auto'

  const option = {
    tooltip: {
      trigger: 'axis' as const,
      axisPointer: { type: 'cross' as const },
      formatter(params: any) {
        const items = Array.isArray(params) ? params : [params]
        let html = `<div style="font-weight:600;margin-bottom:4px">${items[0]?.axisValue}</div>`
        for (const item of items) {
          html += `<div>${item.marker} ${item.seriesName}: <b>${item.value.toLocaleString()}</b></div>`
        }
        return html
      },
    },
    legend: {
      data: ['请求数'],
      top: 0,
      textStyle: { fontSize: 11 },
    },
    grid: { left: 56, right: 16, top: 32, bottom: 40 },
    xAxis: {
      type: 'category' as const,
      data: labels,
      axisLabel: {
        fontSize: 10,
        color: '#999',
        rotate: isMultiDay ? 30 : 0,
        interval: axisLabelInterval,
      },
      axisTick: { show: false },
      axisLine: { lineStyle: { color: '#eee' } },
    },
    yAxis: {
      type: 'value' as const,
      axisLabel: {
        fontSize: 10,
        color: '#999',
      },
      splitLine: { lineStyle: { color: '#f5f5f5' } },
    },
    series: [
      {
        name: '请求数',
        type: 'line' as const,
        data: requestData,
        smooth: true,
        symbol: 'circle',
        symbolSize: 6,
        lineStyle: { width: 2.5, color: COLORS.trace },
        itemStyle: { color: COLORS.trace },
        areaStyle: {
          color: {
            type: 'linear' as const,
            x: 0, y: 0, x2: 0, y2: 1,
            colorStops: [
              { offset: 0, color: 'rgba(139, 92, 246, 0.25)' },
              { offset: 1, color: 'rgba(139, 92, 246, 0.02)' },
            ],
          },
        },
      },
    ],
  }

  return (
    <div>
      <div className="chart-toolbar" style={{ display: 'flex', gap: 8, alignItems: 'center', marginBottom: 8 }}>
        <Segmented
          size="small"
          options={[
            { label: '号池', value: 'pool' },
            { label: '访问', value: 'access' },
          ]}
          value={dimension}
          onChange={(v) => setDimension(v as 'pool' | 'access')}
        />
        {currentKeys && currentKeys.length > 0 && (
          <Select
            allowClear
            placeholder="筛选 Key"
            style={{ width: 160 }}
            value={keyId}
            onChange={(v) => setKeyId(v)}
            size="small"
            options={currentKeys.map((k: any) => ({
              label: k.name || `#${k[keyIdField]}`,
              value: k[keyIdField],
            }))}
          />
        )}
      </div>
      <Spin spinning={loading}>
        <ReactECharts
          option={option}
          style={{ height: 350 }}
          opts={{ renderer: 'canvas' }}
        />
      </Spin>
    </div>
  )
}
