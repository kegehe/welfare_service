<script setup lang="ts">
import { ref, watch } from 'vue'
import { getPoolKeyStats, getAccessKeyStats, getStatsOverview } from '@/api/admin'
import UsageTable from './UsageTable.vue'
import UsageChart from './UsageChart.vue'
import type { PoolKeyStats, AccessKeyStats, StatsOverview } from '@/types'

const hours = ref(24)
const loading = ref(false)
const poolStats = ref<PoolKeyStats[]>([])
const accessStats = ref<AccessKeyStats[]>([])
const overview = ref<StatsOverview | null>(null)

const chartRef = ref<InstanceType<typeof UsageChart>>()

async function loadStats() {
  loading.value = true
  try {
    const [poolData, accessData, overviewData] = await Promise.all([
      getPoolKeyStats(hours.value),
      getAccessKeyStats(hours.value),
      getStatsOverview(hours.value),
    ])
    poolStats.value = poolData.keys || []
    accessStats.value = accessData.keys || []
    overview.value = overviewData
  } catch {
    // ignore
  } finally {
    loading.value = false
  }
}

watch(hours, loadStats)
loadStats()
</script>

<template>
  <div class="usage-stats-section" v-loading="loading">
    <div class="usage-stats-header">
      <span class="usage-stats-title">📊 用量统计</span>
      <el-radio-group v-model="hours" size="small">
        <el-radio-button :value="24">24h</el-radio-button>
        <el-radio-button :value="168">7d</el-radio-button>
        <el-radio-button :value="720">30d</el-radio-button>
      </el-radio-group>
    </div>

    <!-- 概览卡片 -->
    <div v-if="overview" class="overview-row">
      <div class="overview-item">
        <span class="overview-value">{{ overview.total_requests }}</span>
        <span class="overview-label">总请求数</span>
      </div>
      <div class="overview-item">
        <span class="overview-value">{{ overview.active_pool_keys }}</span>
        <span class="overview-label">活跃号池 Key</span>
      </div>
      <div class="overview-item">
        <span class="overview-value">{{ overview.active_access_keys }}</span>
        <span class="overview-label">活跃访问 Key</span>
      </div>
    </div>

    <!-- 双列统计表格 -->
    <el-row :gutter="16" class="usage-tables-row">
      <el-col :xs="24" :sm="12">
        <UsageTable
          title="号池 Key 用量"
          :rows="poolStats"
          name-key="name"
        />
      </el-col>
      <el-col :xs="24" :sm="12">
        <UsageTable
          title="访问 Key 用量"
          :rows="accessStats"
          name-key="name"
          :show-total="true"
        />
      </el-col>
    </el-row>

    <!-- 趋势图表 -->
    <UsageChart ref="chartRef" :hours="hours" />
  </div>
</template>

<style scoped>
.usage-stats-section {
  margin-bottom: 24px;
}

.usage-stats-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}

.usage-stats-title {
  font-size: var(--text-base);
  font-weight: var(--weight-semibold);
  color: var(--text);
}

.overview-row {
  display: flex;
  gap: 24px;
  margin-bottom: 12px;
  padding: 8px 16px;
  background: var(--el-bg-color);
  border-radius: var(--el-border-radius-base);
  border: 1px solid var(--el-border-color-lighter);
}

.overview-item {
  display: flex;
  flex-direction: column;
  align-items: center;
}

.overview-value {
  font-family: var(--font-mono);
  font-size: var(--text-lg);
  font-weight: var(--weight-bold);
  color: var(--ws-pool);
}

.overview-label {
  font-size: var(--text-xs);
  color: var(--text3);
  margin-top: 2px;
}

.usage-tables-row {
  margin-bottom: 16px;
}
</style>
