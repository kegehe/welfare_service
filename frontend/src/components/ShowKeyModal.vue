<script setup lang="ts">
import { useEffectiveBaseUrl } from '@/composables/useBaseUrl'
import { copyTextWithMessage } from '@/composables/useCopyText'

const modelValue = defineModel<boolean>({ required: true })

const props = defineProps<{
  newKey: string
  baseUrl: string
  availableModels: string[]
}>()

const { claudeBaseUrl, codexBaseUrl } = useEffectiveBaseUrl(() => props.baseUrl)

function copyText(text: string, label: string) {
  copyTextWithMessage(text, `${label}已复制`)
}

function copyKey() {
  copyText(props.newKey, 'Key')
}

function copyClaudeUrl() {
  copyText(claudeBaseUrl.value, 'Claude Code Base URL')
}

function copyCodexUrl() {
  copyText(codexBaseUrl.value, 'Codex Base URL')
}

function copyModel(model: string) {
  copyText(model, '模型名')
}

function copyClaudeConfig() {
  const text = `ANTHROPIC_BASE_URL=${claudeBaseUrl.value}\nANTHROPIC_API_KEY=${props.newKey}`
  copyText(text, 'Claude Code 配置')
}

function copyCodexConfig() {
  const text = `OPENAI_BASE_URL=${codexBaseUrl.value}\nOPENAI_API_KEY=${props.newKey}`
  copyText(text, 'Codex 配置')
}
</script>

<template>
  <el-dialog v-model="modelValue" title="🎉 访问 Key 创建成功" width="560px" destroy-on-close>
    <el-result icon="success" title="Key 创建成功" sub-title="请将以下 Key 和配置信息分发给用户">
      <template #extra>
        <!-- Key 显示 -->
        <div class="new-key-display" @click="copyKey">
          {{ newKey }}
        </div>
        <el-text type="info" size="small" class="copy-tip">点击 Key 即可复制</el-text>
        <el-button type="primary" class="copy-btn" @click="copyKey">📋 复制 Key</el-button>

        <el-divider>接入配置</el-divider>

        <div class="config-section">
          <div class="config-header">
            <span class="config-badge config-badge--model">上游模型</span>
            <span class="config-protocol">客户端可请求任意模型，服务会自动映射到活跃号池模型</span>
          </div>
          <div v-if="availableModels.length > 0" class="model-list">
            <el-tag
              v-for="model in availableModels"
              :key="model"
              :title="model"
              size="small"
              type="info"
              class="model-chip"
              @click="copyModel(model)"
            >
              {{ model }}
            </el-tag>
          </div>
          <el-text v-else type="warning" size="small">
            暂无上游模型，请先启用号池 Key 并配置模型列表
          </el-text>
        </div>

        <!-- Claude Code 模式 -->
        <div class="config-section">
          <div class="config-header">
            <span class="config-badge config-badge--claude">Claude Code</span>
            <span class="config-protocol">Claude Messages API</span>
          </div>
          <div class="config-block">
            <div class="config-line">
              <span class="config-label">ANTHROPIC_BASE_URL</span>
              <code class="config-value" @click="copyClaudeUrl">{{ claudeBaseUrl }}</code>
              <el-button size="small" text class="config-copy-btn" @click="copyClaudeUrl">📋</el-button>
            </div>
            <div class="config-line">
              <span class="config-label">ANTHROPIC_API_KEY</span>
              <code class="config-value" @click="copyKey">{{ newKey }}</code>
              <el-button size="small" text class="config-copy-btn" @click="copyKey">📋</el-button>
            </div>
          </div>
          <el-button size="small" type="success" plain class="config-copy-all" @click="copyClaudeConfig">
            复制完整配置
          </el-button>
        </div>

        <!-- Codex 模式 -->
        <div class="config-section">
          <div class="config-header">
            <span class="config-badge config-badge--codex">Codex</span>
            <span class="config-protocol">OpenAI Chat Completions API</span>
          </div>
          <div class="config-block">
            <div class="config-line">
              <span class="config-label">OPENAI_BASE_URL</span>
              <code class="config-value" @click="copyCodexUrl">{{ codexBaseUrl }}</code>
              <el-button size="small" text class="config-copy-btn" @click="copyCodexUrl">📋</el-button>
            </div>
            <div class="config-line">
              <span class="config-label">OPENAI_API_KEY</span>
              <code class="config-value" @click="copyKey">{{ newKey }}</code>
              <el-button size="small" text class="config-copy-btn" @click="copyKey">📋</el-button>
            </div>
          </div>
          <el-button size="small" type="success" plain class="config-copy-all" @click="copyCodexConfig">
            复制完整配置
          </el-button>
        </div>
      </template>
    </el-result>
  </el-dialog>
</template>

<style scoped>
.new-key-display {
  background: var(--key-bg);
  padding: 16px;
  border-radius: var(--radius);
  font-family: var(--font-mono);
  font-size: var(--text-lg);
  word-break: break-all;
  cursor: pointer;
  border: 2px solid var(--ws-pool);
  margin-bottom: 8px;
}

.copy-tip {
  display: block;
  margin-top: 8px;
}

.copy-btn {
  margin-top: 16px;
}

.config-section {
  margin-top: 16px;
  text-align: left;
}

.config-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}

.config-badge {
  display: inline-block;
  padding: 2px 10px;
  border-radius: 4px;
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  color: #fff;
}

.config-badge--claude {
  background: #d97706;
}

.config-badge--codex {
  background: #0d9488;
}

.config-badge--model {
  background: var(--info);
}

.config-protocol {
  font-size: var(--text-xs);
  color: var(--text3);
}

.model-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.model-chip {
  max-width: 100%;
  height: auto;
  min-height: 24px;
  line-height: 1.4;
  white-space: normal;
  word-break: break-all;
  cursor: pointer;
}

.config-block {
  background: var(--surface-muted);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 12px;
}

.config-line {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}

.config-line:last-child {
  margin-bottom: 0;
}

.config-label {
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  color: var(--text2);
  white-space: nowrap;
  min-width: 160px;
}

.config-value {
  flex: 1;
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  color: var(--text);
  background: var(--key-bg);
  padding: 2px 8px;
  border-radius: 4px;
  word-break: break-all;
  cursor: pointer;
  border: 1px solid var(--key-border);
  transition: background 0.15s;
}

.config-value:hover {
  background: var(--key-hover-bg);
}

.config-copy-btn {
  flex-shrink: 0;
  padding: 2px 4px !important;
}

.config-copy-all {
  margin-top: 8px;
}
</style>
