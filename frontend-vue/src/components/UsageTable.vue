<script setup lang="ts">
import { computed } from 'vue'

interface StatsRow {
  total_requests: number
  total_prompt_tokens: number
  total_completion_tokens: number
}

const props = defineProps<{
  title: string
  rows: StatsRow[]
  nameKey: string
  showTotal?: boolean
}>()

const total = computed(() => {
  const t = { total_requests: 0, total_prompt_tokens: 0, total_completion_tokens: 0 }
  for (const row of props.rows) {
    t.total_requests += row.total_requests
    t.total_prompt_tokens += row.total_prompt_tokens
    t.total_completion_tokens += row.total_completion_tokens
  }
  return t
})

function formatTokens(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M'
  if (n >= 1_000) return (n / 1_000).toFixed(1) + 'K'
  return String(n)
}
</script>

<template>
  <div class="usage-table-card">
    <div class="usage-table-title">{{ title }}</div>
    <el-table :data="rows" size="small" stripe :max-height="260">
      <el-table-column :prop="nameKey" label="名称" min-width="80">
        <template #default="{ row }">
          <span class="cell-name">{{ (row as any)[nameKey] || '-' }}</span>
        </template>
      </el-table-column>
      <el-table-column prop="total_requests" label="请求数" width="80" align="right">
        <template #default="{ row }">
          <span class="cell-mono">{{ row.total_requests }}</span>
        </template>
      </el-table-column>
      <el-table-column prop="total_prompt_tokens" label="输入" width="70" align="right">
        <template #default="{ row }">
          <span class="cell-mono">{{ formatTokens(row.total_prompt_tokens) }}</span>
        </template>
      </el-table-column>
      <el-table-column prop="total_completion_tokens" label="输出" width="70" align="right">
        <template #default="{ row }">
          <span class="cell-mono">{{ formatTokens(row.total_completion_tokens) }}</span>
        </template>
      </el-table-column>
    </el-table>
    <div v-if="showTotal && rows.length > 0" class="usage-table-total">
      <span class="total-label">合计</span>
      <span class="total-val">{{ total.total_requests }}</span>
      <span class="total-val">{{ formatTokens(total.total_prompt_tokens) }}</span>
      <span class="total-val">{{ formatTokens(total.total_completion_tokens) }}</span>
    </div>
  </div>
</template>

<style scoped>
.usage-table-card {
  background: var(--el-bg-color);
  border-radius: var(--el-border-radius-base);
  border: 1px solid var(--el-border-color-lighter);
  padding: 12px;
}

.usage-table-title {
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  color: var(--text2);
  margin-bottom: 8px;
}

.cell-name {
  font-size: var(--text-sm);
  color: var(--text);
}

.cell-mono {
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  color: var(--text2);
}

.usage-table-total {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 12px;
  padding: 6px 0 2px;
  border-top: 1px dashed var(--el-border-color-lighter);
  margin-top: 4px;
}

.total-label {
  font-size: var(--text-xs);
  font-weight: var(--weight-semibold);
  color: var(--text2);
  margin-right: auto;
}

.total-val {
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  font-weight: var(--weight-semibold);
  color: var(--ws-pool);
  width: 70px;
  text-align: right;
}
</style>
