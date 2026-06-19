<script setup lang="ts">
import { ref } from 'vue'
import type { AccessKey } from '@/types'
import { useEffectiveBaseUrl } from '@/composables/useBaseUrl'
import { copyTextWithMessage } from '@/composables/useCopyText'

const props = defineProps<{
  accessKeys: AccessKey[]
  baseUrl: string
  availableModels: string[]
}>()

const emit = defineEmits<{
  delete: [id: number]
  toggle: [id: number]
  edit: [key: AccessKey]
  create: []
}>()

const { claudeBaseUrl, codexBaseUrl } = useEffectiveBaseUrl(() => props.baseUrl)

// 使用说明折叠状态
const guideExpanded = ref(false)

function copyKey(key: string) {
  copyTextWithMessage(key)
}

function copyText(text: string, label: string) {
  copyTextWithMessage(text, `${label}已复制`)
}

function copyClaudeUrl() {
  copyText(claudeBaseUrl.value, 'Base URL')
}

function copyCodexUrl() {
  copyText(codexBaseUrl.value, 'Base URL')
}

function copyModel(model: string) {
  copyText(model, '模型名')
}

function copyClaudeConfig() {
  const text = [
    '# Claude Code 接入配置',
    `ANTHROPIC_BASE_URL=${claudeBaseUrl.value}`,
    'ANTHROPIC_API_KEY=<替换为你的访问Key>',
  ].join('\n')
  copyText(text, 'Claude Code 配置')
}

function copyCodexConfig() {
  const text = [
    '# Codex 接入配置',
    `OPENAI_BASE_URL=${codexBaseUrl.value}`,
    'OPENAI_API_KEY=<替换为你的访问Key>',
  ].join('\n')
  copyText(text, 'Codex 配置')
}

function getLimitStr(key: AccessKey): string {
  const tpm = key.tpm_limit || 0
  const rpm = key.rpm_limit || 0
  return (tpm === 0 && rpm === 0) ? '不限' : `T:${tpm}/R:${rpm}`
}

function fmtDate(d: string | null): string {
  if (!d) return '-'
  return d.replace('T', ' ').substring(0, 19)
}

function getStatusTagType(status: string): '' | 'success' | 'danger' {
  return status === 'active' ? 'success' : 'danger'
}
</script>

<template>
  <el-card shadow="hover" class="section-card">
    <template #header>
      <div class="card-header">
        <div>
          <span class="card-title">访问 Key</span>
          <span class="card-desc">用户用来访问号池的凭证，格式 sk-xxx</span>
        </div>
        <div class="card-actions">
          <el-button size="small" text :class="{ 'guide-toggle--active': guideExpanded }" @click="guideExpanded = !guideExpanded">
            📖 {{ guideExpanded ? '收起说明' : '接入说明' }}
          </el-button>
          <el-button type="success" @click="emit('create')">+ 创建访问 Key</el-button>
        </div>
      </div>
    </template>

    <!-- 接入说明 -->
    <el-collapse-transition>
      <div v-show="guideExpanded" class="usage-guide">
        <div class="guide-grid">
          <div class="guide-models">
            <div class="guide-card-header">
              <span class="guide-badge guide-badge--model">上游模型</span>
              <span class="guide-protocol">客户端可请求任意模型，服务会自动映射到活跃号池模型</span>
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

          <!-- Claude Code -->
          <div class="guide-card guide-card--claude">
            <div class="guide-card-header">
              <span class="guide-badge guide-badge--claude">Claude Code</span>
              <span class="guide-protocol">Claude Messages API</span>
            </div>
            <div class="guide-config">
              <div class="guide-line">
                <span class="guide-env">ANTHROPIC_BASE_URL</span>
                <code class="guide-value" @click="copyClaudeUrl">{{ claudeBaseUrl }}</code>
              </div>
              <div class="guide-line">
                <span class="guide-env">ANTHROPIC_API_KEY</span>
                <code class="guide-value guide-value--placeholder">&lt;你的访问Key&gt;</code>
              </div>
            </div>
            <el-button size="small" type="warning" plain @click="copyClaudeConfig">复制配置模板</el-button>
          </div>

          <!-- Codex -->
          <div class="guide-card guide-card--codex">
            <div class="guide-card-header">
              <span class="guide-badge guide-badge--codex">Codex</span>
              <span class="guide-protocol">OpenAI Chat Completions API</span>
            </div>
            <div class="guide-config">
              <div class="guide-line">
                <span class="guide-env">OPENAI_BASE_URL</span>
                <code class="guide-value" @click="copyCodexUrl">{{ codexBaseUrl }}</code>
              </div>
              <div class="guide-line">
                <span class="guide-env">OPENAI_API_KEY</span>
                <code class="guide-value guide-value--placeholder">&lt;你的访问Key&gt;</code>
              </div>
            </div>
            <el-button size="small" type="success" plain @click="copyCodexConfig">复制配置模板</el-button>
          </div>
        </div>
      </div>
    </el-collapse-transition>

    <el-table :data="accessKeys" stripe class="full-table" empty-text="暂无访问 Key，创建一个分发给用户">
      <el-table-column prop="id" label="ID" width="60" />
      <el-table-column label="Key" width="260">
        <template #default="{ row }">
          <code class="sk-key" @click="copyKey(row.key)">
            {{ row.key }}
            <span class="copy-hint">📋点击复制</span>
          </code>
        </template>
      </el-table-column>
      <el-table-column label="名称" width="140">
        <template #default="{ row }">
          {{ row.name || '-' }}
        </template>
      </el-table-column>
      <el-table-column label="限流" width="110">
        <template #default="{ row }">
          {{ getLimitStr(row) }}
        </template>
      </el-table-column>
      <el-table-column label="过期" width="170">
        <template #default="{ row }">
          {{ fmtDate(row.expires_at) }}
        </template>
      </el-table-column>
      <el-table-column label="最后使用" width="170">
        <template #default="{ row }">
          {{ fmtDate(row.last_used_at) }}
        </template>
      </el-table-column>
      <el-table-column label="状态" width="80">
        <template #default="{ row }">
          <el-tag :type="getStatusTagType(row.status)" size="small">
            {{ row.status === 'active' ? '活跃' : '禁用' }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column label="操作" width="220" fixed="right">
        <template #default="{ row }">
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

.card-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}

.guide-toggle--active {
  color: var(--ws-pool) !important;
}

/* ── Usage Guide ── */
.usage-guide {
  margin-bottom: 16px;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--border);
}

.guide-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
}

@media (max-width: 768px) {
  .guide-grid {
    grid-template-columns: 1fr;
  }
}

.guide-card {
  background: var(--surface-muted);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 16px;
}

.guide-models {
  grid-column: 1 / -1;
  background: var(--surface-muted);
  border: 1px solid var(--border);
  border-left: 3px solid var(--info);
  border-radius: var(--radius);
  padding: 16px;
}

.guide-card--claude {
  border-left: 3px solid #d97706;
}

.guide-card--codex {
  border-left: 3px solid var(--ws-pool);
}

.guide-card-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}

.guide-badge {
  display: inline-block;
  padding: 2px 10px;
  border-radius: 4px;
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  color: #fff;
}

.guide-badge--claude {
  background: #d97706;
}

.guide-badge--codex {
  background: var(--ws-pool);
}

.guide-badge--model {
  background: var(--info);
}

.guide-protocol {
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

.guide-config {
  background: var(--card);
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 10px 12px;
  margin-bottom: 10px;
}

.guide-line {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}

.guide-line:last-child {
  margin-bottom: 0;
}

.guide-env {
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  color: var(--text2);
  white-space: nowrap;
  min-width: 160px;
}

.guide-value {
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

.guide-value:hover {
  background: var(--key-hover-bg);
}

/* 占位符样式：视觉上区分实际值和占位文本 */
.guide-value--placeholder {
  color: var(--text3);
  background: var(--surface-muted);
  border-style: dashed;
  cursor: default;
}

.guide-value--placeholder:hover {
  background: var(--surface-muted);
}

/* ── Key Display ── */
.sk-key {
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  cursor: pointer;
  background: var(--key-bg);
  padding: 2px 6px;
  border-radius: var(--radius);
  border: 1px solid var(--key-border);
  position: relative;
  display: inline-block;
  transition: background 0.15s;
}

.sk-key:hover {
  background: var(--key-hover-bg);
}

.copy-hint {
  font-size: var(--text-xs);
  color: var(--primary);
  opacity: 0;
  transition: opacity 0.3s;
  margin-left: 4px;
}

.sk-key:hover .copy-hint {
  opacity: 1;
}
</style>
