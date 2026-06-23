<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  score: number        // 0-100
  statusLabel: string  // 'normal' | 'light_throttled' | 'heavy_throttled' | 'key_invalid' | 'nodata'
}>()

const COLOR_MAP: Record<string, string> = {
  normal: '#22C55E',
  light_throttled: '#84CC16',
  heavy_throttled: '#F59E0B',
  key_invalid: '#EF4444',
  nodata: '#E5E7EB',
}

const STATUS_TEXT_COLOR: Record<string, string> = {
  normal: '#22C55E',
  light_throttled: '#84CC16',
  heavy_throttled: '#F59E0B',
  key_invalid: '#EF4444',
  nodata: '#9CA3AF',
}

const LABEL_MAP: Record<string, string> = {
  normal: '正常稳定',
  light_throttled: '轻度限流，偶发429',
  heavy_throttled: '重度限流，大量超时',
  key_invalid: '密钥失效/封禁/401',
  nodata: '无数据',
}

const fillColor = computed(() => COLOR_MAP[props.statusLabel] || '#E5E7EB')
const emptyColor = '#E5E7EB'
const statusText = computed(() => LABEL_MAP[props.statusLabel] || '无数据')
const statusTextColor = computed(() => STATUS_TEXT_COLOR[props.statusLabel] || '#9CA3AF')
</script>

<template>
  <div class="health-score-grid">
    <div class="grid-bar">
      <div
        v-for="i in 100"
        :key="i"
        class="grid-cell"
        :style="{ backgroundColor: i - 1 < score ? fillColor : emptyColor }"
      />
    </div>
    <div class="grid-label" :style="{ color: statusTextColor }">
      {{ statusText }}
    </div>
  </div>
</template>

<style scoped>
.health-score-grid {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 4px;
}

.grid-bar {
  display: grid;
  grid-template-columns: repeat(100, 2px);
  gap: 1px;
}

.grid-cell {
  width: 2px;
  height: 10px;
  border-radius: 1px;
}

.grid-label {
  text-align: center;
  font-size: 12px;
  font-weight: 500;
  line-height: 1;
  white-space: nowrap;
}
</style>
