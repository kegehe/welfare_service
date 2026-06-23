<script setup lang="ts">
import { ref } from 'vue'
import type { PoolKey, KeyStatus, KeyHealthScore } from '@/types'
import HealthScoreGrid from '@/components/HealthScoreGrid.vue'

const props = defineProps<{
  poolKeys: PoolKey[]
  statusMap: Record<number, KeyStatus>
  healthScoreMap: Record<number, KeyHealthScore>
  testingKeyId: number | null
}>()

const emit = defineEmits<{
  delete: [id: number]
  toggle: [id: number]
  test: [id: number]
  edit: [key: PoolKey]
  add: []
}>()

const viewMode = ref<'card' | 'table'>('card')

function getStatus(key: PoolKey): KeyStatus | undefined {
  return props.statusMap[key.id]
}

function getHealthScore(key: PoolKey): KeyHealthScore | undefined {
  return props.healthScoreMap[key.id]
}

function getHealthScoreValue(key: PoolKey): number {
  const hs = getHealthScore(key)
  return hs ? hs.health_score : 0
}

function getHealthStatusLabel(key: PoolKey): string {
  const hs = getHealthScore(key)
  return hs ? hs.status_label : 'nodata'
}

function getLimitStr(key: PoolKey): string {
  const tpm = key.tpm_limit || 0
  const rpm = key.rpm_limit || 0
  return (tpm === 0 && rpm === 0) ? '不限' : `T:${tpm}/R:${rpm}`
}

function getCircuitTagType(state: string): '' | 'success' | 'warning' | 'danger' {
  if (state === 'closed') return 'success'
  if (state === 'open') return 'danger'
  if (state === 'half_open') return 'warning'
  return ''
}

function getStatusTagType(status: string): '' | 'success' | 'danger' {
  return status === 'active' ? 'success' : 'danger'
}

function getStatusLabel(status: string): string {
  if (status === 'active') return '活跃'
  if (status === 'unhealthy') return '异常'
  return '禁用'
}

function getCircuitLabel(state: string): string {
  if (state === 'closed') return '正常'
  if (state === 'open') return '熔断'
  if (state === 'half_open') return '半开'
  return state
}

function getRemainingValue(value: number | undefined): string {
  if (value == null || value < 0) return '-'
  return String(value)
}

function getPlatformLabel(key: PoolKey): string {
  const map: Record<string, string> = {
    xiaomi: '小米',
    iflytek: '讯飞',
    anthropic: 'Anthropic',
  }
  return map[key.platform] || key.platform || '未命名平台'
}

function getKeyTitle(key: PoolKey): string {
  return key.name?.trim() || getPrimaryModel(key)
}

function getPrimaryModel(key: PoolKey): string {
  return getDisplayModels(key)[0] || '未配置模型'
}

function getDisplayModels(key: PoolKey): string[] {
  return key.models.map(model => model.trim()).filter(Boolean)
}
</script>

<template>
  <el-card shadow="hover" class="section-card">
    <template #header>
      <div class="card-header">
        <div>
          <span class="card-title">号池 Key</span>
          <span class="card-desc">上游平台 API Key，代理请求时自动从中选取</span>
        </div>
        <div class="header-actions">
          <el-segmented
            v-model="viewMode"
            :options="[
              { label: '卡片', value: 'card' },
              { label: '表格', value: 'table' },
            ]"
            size="small"
          />
          <el-button type="primary" @click="emit('add')">+ 添加号池 Key</el-button>
        </div>
      </div>
    </template>

    <el-empty v-if="poolKeys.length === 0" description="暂无号池 Key">
      <el-button type="primary" @click="emit('add')">添加号池 Key</el-button>
    </el-empty>

    <div v-else-if="viewMode === 'card'" class="pool-card-grid">
      <article
        v-for="key in poolKeys"
        :key="key.id"
        class="pool-key-card"
        :class="[
          `pool-key-card--${key.status}`,
          `pool-key-card--circuit-${getStatus(key)?.circuit_state || 'closed'}`,
        ]"
      >
        <header class="pool-card-header">
          <div class="pool-card-title-block">
            <div class="pool-card-kicker">
              <span class="platform-dot" aria-hidden="true"></span>
              <span>{{ getPlatformLabel(key) }}</span>
              <span class="pool-card-id">#{{ key.id }}</span>
            </div>
            <h3 class="pool-card-title">{{ getKeyTitle(key) }}</h3>
            <code class="key-prefix pool-card-key">{{ key.key_prefix }}</code>
          </div>
          <div class="pool-card-tags">
            <el-tag :type="getStatusTagType(key.status)" size="small">
              {{ getStatusLabel(key.status) }}
            </el-tag>
            <el-tag :type="getCircuitTagType(getStatus(key)?.circuit_state || 'closed')" size="small">
              {{ getCircuitLabel(getStatus(key)?.circuit_state || 'closed') }}
            </el-tag>
          </div>
        </header>

        <section class="health-panel">
          <div class="health-rate">
            <HealthScoreGrid
              :score="getHealthScoreValue(key)"
              :status-label="getHealthStatusLabel(key)"
            />
          </div>
          <div class="metric-strip">
            <div class="metric-item">
              <span class="metric-label">TPM 限制</span>
              <strong>{{ key.tpm_limit || '不限' }}</strong>
            </div>
            <div class="metric-item">
              <span class="metric-label">RPM 限制</span>
              <strong>{{ key.rpm_limit || '不限' }}</strong>
            </div>
            <div class="metric-item">
              <span class="metric-label">TPM 剩余</span>
              <strong>{{ getRemainingValue(getStatus(key)?.tpm_remaining) }}</strong>
            </div>
            <div class="metric-item">
              <span class="metric-label">RPM 剩余</span>
              <strong>{{ getRemainingValue(getStatus(key)?.rpm_remaining) }}</strong>
            </div>
          </div>
        </section>

        <section class="pool-card-details">
          <div class="detail-row">
            <span class="detail-label">名称</span>
            <span class="detail-value">{{ key.name || '-' }}</span>
          </div>
          <div class="detail-row detail-row--models">
            <span class="detail-label">模型</span>
            <div class="models-list model-chip-list">
              <el-tag
                v-for="m in getDisplayModels(key)"
                :key="m"
                size="small"
                type="info"
                class="model-tag"
              >
                {{ m }}
              </el-tag>
              <span v-if="getDisplayModels(key).length === 0" class="muted-text">未配置</span>
            </div>
          </div>
          <div class="detail-meta-grid">
            <div>
              <span class="detail-label">来源</span>
              <span class="detail-value">{{ key.source || '-' }}</span>
            </div>
            <div>
              <span class="detail-label">创建时间</span>
              <span class="detail-value">{{ key.created_at || '-' }}</span>
            </div>
          </div>
          <div v-if="key.note" class="note-box">{{ key.note }}</div>
        </section>

        <footer class="pool-card-actions">
          <el-button
            :loading="testingKeyId === key.id"
            @click="emit('test', key.id)"
          >
            测试
          </el-button>
          <el-button type="primary" plain @click="emit('edit', key)">
            编辑
          </el-button>
          <el-button @click="emit('toggle', key.id)">
            {{ key.status === 'active' ? '禁用' : '启用' }}
          </el-button>
          <el-button type="danger" plain @click="emit('delete', key.id)">
            删除
          </el-button>
        </footer>
      </article>
    </div>

    <el-table v-else :data="poolKeys" stripe class="full-table" empty-text="暂无号池 Key">
      <el-table-column prop="id" label="ID" width="60" />
      <el-table-column prop="platform" label="平台" min-width="80" />
      <el-table-column label="名称" min-width="120">
        <template #default="{ row }">
          {{ row.name || '-' }}
        </template>
      </el-table-column>
      <el-table-column label="密钥" min-width="100">
        <template #default="{ row }">
          <code class="key-prefix">{{ row.key_prefix }}</code>
        </template>
      </el-table-column>
      <el-table-column label="模型" min-width="160">
        <template #default="{ row }">
          <div class="models-list">
            <el-tag v-for="m in getDisplayModels(row)" :key="m" size="small" type="info" class="model-tag">{{ m }}</el-tag>
            <span v-if="getDisplayModels(row).length === 0" class="muted-text">未配置</span>
          </div>
        </template>
      </el-table-column>
      <el-table-column label="限流" min-width="90">
        <template #default="{ row }">
          {{ getLimitStr(row) }}
        </template>
      </el-table-column>
      <el-table-column label="健康评分" width="310">
        <template #default="{ row }">
          <HealthScoreGrid
            :score="getHealthScoreValue(row)"
            :status-label="getHealthStatusLabel(row)"
          />
        </template>
      </el-table-column>
      <el-table-column label="熔断器" min-width="80">
        <template #default="{ row }">
          <el-tag :type="getCircuitTagType(getStatus(row)?.circuit_state || 'closed')" size="small">
            {{ getCircuitLabel(getStatus(row)?.circuit_state || 'closed') }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column label="状态" min-width="70">
        <template #default="{ row }">
          <el-tag :type="getStatusTagType(row.status)" size="small">
            {{ getStatusLabel(row.status) }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column label="操作" width="240" fixed="right">
        <template #default="{ row }">
          <el-button
            size="small"
            :loading="testingKeyId === row.id"
            @click="emit('test', row.id)"
          >
            🔍 测试
          </el-button>
          <el-button size="small" type="primary" plain @click="emit('edit', row)">
            编辑
          </el-button>
          <el-button size="small" @click="emit('toggle', row.id)">
            {{ row.status === 'active' ? '禁用' : '启用' }}
          </el-button>
          <el-button size="small" type="danger" @click="emit('delete', row.id)">
            删除
          </el-button>
        </template>
      </el-table-column>
    </el-table>
  </el-card>
</template>

<style scoped>
.full-table { width: 100%; }

.header-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

.key-prefix {
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  color: var(--text2);
}

.models-list {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.model-tag {
  font-size: var(--text-xs);
}

.pool-card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 16px;
}

.pool-key-card {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-width: 0;
  padding: 18px;
  border: 1px solid var(--border);
  border-left: 4px solid var(--ws-pool);
  border-radius: 8px;
  background:
    linear-gradient(135deg, rgba(var(--ws-pool-rgb), 0.08), rgba(255, 255, 255, 0) 32%),
    var(--card);
  box-shadow: 0 10px 24px rgba(var(--ws-conduit-rgb), 0.06);
}

.pool-key-card--disabled,
.pool-key-card--expired {
  border-left-color: var(--info);
}

.pool-key-card--unhealthy,
.pool-key-card--circuit-open {
  border-left-color: var(--danger);
}

.pool-key-card--circuit-half_open {
  border-left-color: var(--warning);
}

.pool-card-header,
.pool-card-actions {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 14px;
}

.pool-card-title-block {
  min-width: 0;
}

.pool-card-kicker {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
  color: var(--text2);
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
}

.platform-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--ws-pool);
  box-shadow: 0 0 0 3px rgba(var(--ws-pool-rgb), 0.14);
}

.pool-card-id {
  color: var(--text3);
  font-family: var(--font-mono);
}

.pool-card-title {
  margin: 0 0 8px;
  color: var(--text);
  font-size: var(--text-xl);
  font-weight: var(--weight-semibold);
  line-height: 1.25;
  overflow-wrap: anywhere;
}

.pool-card-key {
  display: inline-flex;
  max-width: 100%;
  padding: 4px 8px;
  border: 1px solid var(--key-border);
  border-radius: 6px;
  background: var(--key-bg);
  overflow-wrap: anywhere;
}

.pool-card-tags {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 6px;
}

.health-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 14px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: rgba(var(--ws-channel-rgb), 0.28);
}

.health-rate {
  min-width: 0;
}

.metric-strip {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 8px;
}

.metric-item {
  min-width: 0;
  padding: 8px 10px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--card);
}

.metric-label,
.detail-label {
  display: block;
  color: var(--text3);
  font-size: var(--text-xs);
  font-weight: var(--weight-medium);
}

.metric-item strong {
  display: block;
  margin-top: 2px;
  color: var(--text);
  font-size: var(--text-base);
  font-weight: var(--weight-semibold);
  overflow-wrap: anywhere;
}

.pool-card-details {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-width: 0;
}

.detail-row {
  display: grid;
  grid-template-columns: 92px minmax(0, 1fr);
  align-items: start;
  gap: 12px;
}

.detail-row--models {
  align-items: center;
}

.detail-value {
  min-width: 0;
  color: var(--text2);
  font-size: var(--text-sm);
  overflow-wrap: anywhere;
}

.detail-url {
  font-family: var(--font-mono);
}

.detail-meta-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
  padding-top: 2px;
}

.detail-meta-grid .detail-value {
  display: block;
  margin-top: 2px;
}

.model-chip-list {
  min-width: 0;
}

.muted-text {
  color: var(--text3);
  font-size: var(--text-sm);
}

.note-box {
  padding: 10px 12px;
  border-left: 3px solid var(--ws-pool);
  border-radius: 6px;
  background: var(--surface-muted);
  color: var(--text2);
  font-size: var(--text-sm);
  overflow-wrap: anywhere;
}

.pool-card-actions {
  align-items: center;
  justify-content: flex-end;
  padding-top: 2px;
}

.pool-card-actions :deep(.el-button + .el-button) {
  margin-left: 0;
}

@media (max-width: 760px) {
  .card-header,
  .header-actions,
  .pool-card-header,
  .pool-card-actions {
    align-items: stretch;
    flex-direction: column;
  }

  .pool-card-grid {
    grid-template-columns: 1fr;
  }

  .detail-row,
  .detail-meta-grid {
    grid-template-columns: 1fr;
  }

  .metric-strip {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .pool-card-tags {
    justify-content: flex-start;
  }

  .pool-card-actions .el-button {
    width: 100%;
  }
}

@media (max-width: 520px) {
  .metric-strip {
    grid-template-columns: 1fr;
  }

  .pool-key-card {
    padding: 14px;
  }
}
</style>
