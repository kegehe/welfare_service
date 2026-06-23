<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted } from 'vue'
import * as echarts from 'echarts/core'
import { BarChart } from 'echarts/charts'
import { GridComponent, TooltipComponent, LegendComponent, DataZoomComponent } from 'echarts/components'
import { CanvasRenderer } from 'echarts/renderers'
import { getHourlyStats } from '@/api/admin'
import type { HourlyStats } from '@/types'

echarts.use([BarChart, GridComponent, TooltipComponent, LegendComponent, DataZoomComponent, CanvasRenderer])

const props = defineProps<{
  hours: number
}>()

const chartRef = ref<HTMLDivElement>()
let chart: echarts.ECharts | null = null
const loading = ref(false)
const dimension = ref<'pool' | 'access'>('pool')
const keyId = ref<number | undefined>(undefined)

async function loadData() {
  loading.value = true
  try {
    const res = await getHourlyStats(dimension.value, keyId.value, props.hours)
    renderChart(res.data || [])
  } catch {
    // ignore
  } finally {
    loading.value = false
  }
}

/** 从 CSS 自定义属性中解析实际颜色值（ECharts Canvas 不支持 var()） */
function resolveCssVar(varName: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(varName).trim()
}

function renderChart(data: HourlyStats[]) {
  if (!chart) return

  // 按小时聚合
  const hourMap = new Map<number, { prompt: number; completion: number }>()
  for (const row of data) {
    const existing = hourMap.get(row.hour_bucket) || { prompt: 0, completion: 0 }
    existing.prompt += row.prompt_tokens
    existing.completion += row.completion_tokens
    hourMap.set(row.hour_bucket, existing)
  }

  const hours = [...hourMap.keys()].sort()
  const promptData = hours.map(h => hourMap.get(h)!.prompt)
  const completionData = hours.map(h => hourMap.get(h)!.completion)
  const labels = hours.map(h => {
    const d = new Date(h * 3600 * 1000)
    return `${String(d.getHours()).padStart(2, '0')}:00`
  })

  const option: echarts.EChartsCoreOption = {
    tooltip: {
      trigger: 'axis',
      axisPointer: { type: 'shadow' },
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
      type: 'category',
      data: labels,
      axisLabel: { fontSize: 10, color: '#999' },
      axisTick: { show: false },
      axisLine: { lineStyle: { color: '#eee' } }
    },
    yAxis: {
      type: 'value',
      axisLabel: {
        fontSize: 10,
        color: '#999',
        formatter(v: number) { return formatTokens(v) }
      },
      splitLine: { lineStyle: { color: '#f5f5f5' } }
    },
    series: [
      {
        name: '输入 Tokens',
        type: 'bar',
        stack: 'tokens',
        data: promptData,
        itemStyle: { color: resolveCssVar('--ws-pool') || '#409eff' },
        barMaxWidth: 24,
      },
      {
        name: '输出 Tokens',
        type: 'bar',
        stack: 'tokens',
        data: completionData,
        itemStyle: { color: resolveCssVar('--ws-signal') || '#67c23a' },
        barMaxWidth: 24,
      }
    ]
  }

  chart.setOption(option, true)
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M'
  if (n >= 1_000) return (n / 1_000).toFixed(1) + 'K'
  return String(n)
}

function handleResize() {
  chart?.resize()
}

watch(() => props.hours, loadData)

onMounted(() => {
  if (chartRef.value) {
    chart = echarts.init(chartRef.value)
    loadData()
    window.addEventListener('resize', handleResize)
  }
})

onUnmounted(() => {
  window.removeEventListener('resize', handleResize)
  chart?.dispose()
})

defineExpose({ loadData })
</script>

<template>
  <div class="usage-chart-card">
    <div class="usage-chart-header">
      <span class="usage-chart-title">📈 用量趋势</span>
      <div class="usage-chart-controls">
        <el-radio-group v-model="dimension" size="small" @change="loadData">
          <el-radio-button value="pool">号池 Key</el-radio-button>
          <el-radio-button value="access">访问 Key</el-radio-button>
        </el-radio-group>
      </div>
    </div>
    <div v-loading="loading" class="usage-chart-container" ref="chartRef"></div>
  </div>
</template>

<style scoped>
.usage-chart-card {
  background: var(--el-bg-color);
  border-radius: var(--el-border-radius-base);
  border: 1px solid var(--el-border-color-lighter);
  padding: 12px;
}

.usage-chart-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.usage-chart-title {
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  color: var(--text2);
}

.usage-chart-controls {
  display: flex;
  gap: 8px;
  align-items: center;
}

.usage-chart-container {
  width: 100%;
  height: 240px;
}
</style>
