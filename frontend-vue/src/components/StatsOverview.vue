<script setup lang="ts">
import { computed } from 'vue'
import type { PoolKey, KeyStatus, AccessKey } from '@/types'

const props = defineProps<{
  poolKeys: PoolKey[]
  keyStatuses: KeyStatus[]
  accessKeys: AccessKey[]
  version: string
}>()

const poolActive = computed(() => props.poolKeys.filter(k => k.status === 'active').length)
const poolDisabled = computed(() => props.poolKeys.filter(k => k.status !== 'active').length)
const circuitOpen = computed(() => props.keyStatuses.filter(s => s.circuit_state === 'open').length)
const accessActive = computed(() => props.accessKeys.filter(k => k.status === 'active').length)
const accessTotal = computed(() => props.accessKeys.length)
</script>

<template>
  <el-row :gutter="16" class="stats-row">
    <!-- Pool Health cluster -->
    <el-col :xs="24" :sm="12" :md="14">
      <div class="stat-group">
        <div class="stat-group-label">Pool Health</div>
        <el-row :gutter="12">
          <el-col :xs="12" :sm="6">
            <el-card shadow="hover" class="stat-card stat-card--pool">
              <div class="stat-value">{{ poolKeys.length }}</div>
              <div class="stat-label">号池 Key</div>
            </el-card>
          </el-col>
          <el-col :xs="12" :sm="6">
            <el-card shadow="hover" class="stat-card stat-card--signal">
              <div class="stat-value">{{ poolActive }}</div>
              <div class="stat-label">号池活跃</div>
            </el-card>
          </el-col>
          <el-col :xs="12" :sm="6">
            <el-card shadow="hover" class="stat-card stat-card--fault">
              <div class="stat-value">{{ poolDisabled }}</div>
              <div class="stat-label">号池禁用</div>
            </el-card>
          </el-col>
          <el-col :xs="12" :sm="6">
            <el-card shadow="hover" class="stat-card" :class="circuitOpen > 0 ? 'stat-card--alarm' : 'stat-card--ok'">
              <div class="stat-value">{{ circuitOpen }}</div>
              <div class="stat-label">熔断中</div>
            </el-card>
          </el-col>
        </el-row>
      </div>
    </el-col>

    <!-- Access Keys cluster -->
    <el-col :xs="24" :sm="12" :md="10">
      <div class="stat-group">
        <div class="stat-group-label">Access Keys</div>
        <el-row :gutter="12">
          <el-col :span="12">
            <el-card shadow="hover" class="stat-card stat-card--pool">
              <div class="stat-value">{{ accessTotal }}</div>
              <div class="stat-label">访问 Key</div>
            </el-card>
          </el-col>
          <el-col :span="12">
            <el-card shadow="hover" class="stat-card stat-card--signal">
              <div class="stat-value">{{ accessActive }}</div>
              <div class="stat-label">访问活跃</div>
            </el-card>
          </el-col>
        </el-row>
      </div>
    </el-col>
  </el-row>
</template>

<style scoped>
.stats-row {
  margin-bottom: 24px;
}

.stat-group-label {
  font-size: var(--text-xs);
  font-weight: var(--weight-semibold);
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--text3);
  margin-bottom: 8px;
  padding-left: 4px;
}

.stat-card {
  text-align: center;
  border-radius: var(--radius);
  transition: border-color 0.2s;
}

.stat-card .stat-value {
  font-size: var(--text-2xl);
  font-weight: var(--weight-bold);
  font-family: var(--font-mono);
  line-height: 1.2;
  color: var(--text);
}

.stat-card .stat-label {
  font-size: var(--text-sm);
  color: var(--text3);
  margin-top: 4px;
}

/* Color accents via top border */
.stat-card--pool .stat-value { color: var(--ws-pool); }
.stat-card--pool { border-top: 3px solid var(--ws-pool); }

.stat-card--signal .stat-value { color: var(--ws-signal); }
.stat-card--signal { border-top: 3px solid var(--ws-signal); }

.stat-card--fault .stat-value { color: var(--ws-fault); }
.stat-card--fault { border-top: 3px solid var(--ws-fault); }

.stat-card--ok .stat-value { color: var(--ws-signal); }
.stat-card--ok { border-top: 3px solid var(--ws-signal); }

/* Alarm state for circuit-open */
.stat-card--alarm .stat-value { color: var(--ws-fault); }
.stat-card--alarm {
  border-top: 3px solid var(--ws-fault);
  animation: pulse-alarm 2s ease-in-out infinite;
}

@keyframes pulse-alarm {
  0%, 100% { box-shadow: 0 0 0 0 rgba(var(--ws-fault-rgb), 0); }
  50% { box-shadow: 0 0 0 4px rgba(var(--ws-fault-rgb), 0.15); }
}
</style>
