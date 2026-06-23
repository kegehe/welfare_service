<script setup lang="ts">
import { computed } from 'vue'
import type { TestKeyResult } from '@/types'

const modelValue = defineModel<boolean>({ required: true })

const props = defineProps<{
  result: TestKeyResult | null
}>()

type EndpointStatus = { label: string; type: 'success' | 'warning' | 'danger' | 'info'; icon: string }

/** 根据端点测试结果获取状态文本和标签类型 */
function getEndpointStatus(endpoint: { reachable: boolean; key_valid: boolean; upstream_error: boolean; status?: number }): EndpointStatus {
  if (endpoint.key_valid) {
    if (endpoint.status === 429) {
      return { label: '限流中 (Key 有效)', type: 'warning', icon: '⚠' }
    }
    return { label: 'Key 有效', type: 'success', icon: '✓' }
  }
  if (endpoint.upstream_error) {
    // 5xx — 上游临时故障，key 状态未知
    return { label: '上游暂时不可用', type: 'info', icon: '☁' }
  }
  if (endpoint.reachable) {
    // 401/403 — 端点可达但 key 无效
    return { label: 'Key 无效', type: 'danger', icon: '✗' }
  }
  // 404/超时/网络错误 — 端点不可达
  return { label: '端点不可达', type: 'danger', icon: '✗' }
}

const openaiStatus = computed<EndpointStatus | null>(() =>
  props.result?.openai ? getEndpointStatus(props.result.openai) : null,
)

const claudeStatus = computed<EndpointStatus | null>(() =>
  props.result?.claude ? getEndpointStatus(props.result.claude) : null,
)
</script>

<template>
  <el-dialog v-model="modelValue" title="🔍 Key 连通性测试结果" width="560px">
    <template v-if="result">
      <el-result
        :icon="result.available ? 'success' : 'error'"
        :title="result.available ? 'Key 可用' : 'Key 不可用'"
      >
        <template #sub-title>
          <div class="test-details">
            <!-- OpenAI 端点 -->
            <div v-if="result.openai && openaiStatus" class="test-result">
              <strong>OpenAI 端点</strong>
              <el-tag :type="openaiStatus.type" size="small">{{ openaiStatus.icon }} {{ openaiStatus.label }}</el-tag>
              <el-text v-if="result.openai.error" type="danger" size="small">{{ result.openai.error }}</el-text>
              <span class="latency">{{ result.openai.latency_ms }}ms</span>
              <el-text v-if="result.openai.status" type="info" size="small" class="latency">HTTP {{ result.openai.status }}</el-text>
            </div>

            <!-- Claude 端点 -->
            <div v-if="result.claude && claudeStatus" class="test-result">
              <strong>Claude 端点</strong>
              <el-tag :type="claudeStatus.type" size="small">{{ claudeStatus.icon }} {{ claudeStatus.label }}</el-tag>
              <el-text v-if="result.claude.error" type="danger" size="small">{{ result.claude.error }}</el-text>
              <span class="latency">{{ result.claude.latency_ms }}ms</span>
              <el-text v-if="result.claude.status" type="info" size="small" class="latency">HTTP {{ result.claude.status }}</el-text>
            </div>

            <div v-if="result.claude" class="test-hint">
              <el-text type="info" size="small">
                提示：Claude API 不支持 GET 请求，返回 405 即代表端点可达、Key 有效
              </el-text>
            </div>
          </div>
        </template>
      </el-result>
    </template>
  </el-dialog>
</template>

<style scoped>
.test-details {
  text-align: left;
}

.test-result {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  font-size: var(--text-sm);
  padding: 8px;
  border-radius: var(--radius);
  background: var(--surface-muted);
  margin-top: 4px;
}

.test-hint {
  margin-top: 8px;
  padding: 6px 8px;
  border-radius: var(--radius);
  background: var(--surface-muted);
}

.latency {
  color: var(--text3);
  font-size: var(--text-sm);
}
</style>
