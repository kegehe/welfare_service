<script setup lang="ts">
import type { TestKeyResult } from '@/types'

const modelValue = defineModel<boolean>({ required: true })

defineProps<{
  result: TestKeyResult | null
}>()
</script>

<template>
  <el-dialog v-model="modelValue" title="🔍 Key 连通性测试结果" width="520px">
    <template v-if="result">
      <el-result
        :icon="result.available ? 'success' : 'error'"
        :title="result.available ? 'Key 可用' : 'Key 不可用'"
      >
        <template #sub-title>
          <div class="test-details">
            <!-- OpenAI 端点 -->
            <div v-if="result.openai" class="test-result">
              <strong>OpenAI 端点</strong>
              <template v-if="result.openai.success">
                <el-tag type="success" size="small">✓ 可用</el-tag>
                <span class="latency">{{ result.openai.latency_ms }}ms</span>
              </template>
              <template v-else>
                <el-tag type="danger" size="small">✗ 不可用</el-tag>
                <el-text v-if="result.openai.error" type="danger" size="small">{{ result.openai.error }}</el-text>
                <span v-if="result.openai.latency_ms" class="latency">{{ result.openai.latency_ms }}ms</span>
              </template>
              <el-text v-if="result.openai.status" type="info" size="small" class="latency">HTTP {{ result.openai.status }}</el-text>
            </div>

            <!-- Claude 端点 -->
            <div v-if="result.claude" class="test-result">
              <strong>Claude 端点</strong>
              <template v-if="result.claude.success">
                <el-tag type="success" size="small">✓ 可用</el-tag>
                <span class="latency">{{ result.claude.latency_ms }}ms</span>
              </template>
              <template v-else>
                <el-tag type="danger" size="small">✗ 不可用</el-tag>
                <el-text v-if="result.claude.error" type="danger" size="small">{{ result.claude.error }}</el-text>
                <span v-if="result.claude.latency_ms" class="latency">{{ result.claude.latency_ms }}ms</span>
              </template>
              <el-text v-if="result.claude.status" type="info" size="small" class="latency">HTTP {{ result.claude.status }}</el-text>
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

.latency {
  color: var(--text3);
  font-size: var(--text-sm);
}
</style>
