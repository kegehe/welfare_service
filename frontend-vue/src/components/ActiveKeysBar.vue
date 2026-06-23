<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import type { ActiveKeyEntry } from '@/types'

const activeKeys = ref<ActiveKeyEntry[]>([])
const connected = ref(false)
let eventSource: EventSource | null = null
let durationTimer: ReturnType<typeof setInterval> | null = null
// 用于强制刷新持续时间显示的计数器
const tick = ref(0)

function formatDuration(startedAt: number): string {
  tick.value // 依赖 tick 触发重算
  const diff = Math.max(0, Date.now() - startedAt)
  const seconds = Math.floor(diff / 1000)
  if (seconds < 60) return `${seconds}s`
  const minutes = Math.floor(seconds / 60)
  const remainSeconds = seconds % 60
  return `${minutes}m${remainSeconds}s`
}

function getPlatformLabel(platform: string): string {
  const map: Record<string, string> = {
    xiaomi: '小米',
    iflytek: '讯飞',
    anthropic: 'Anthropic',
  }
  return map[platform] || platform || '未命名平台'
}

function getPlatformColor(platform: string): string {
  const map: Record<string, string> = {
    xiaomi: '#ff6900',
    iflytek: '#0066cc',
    anthropic: '#d97706',
  }
  return map[platform] || 'var(--ws-pool)'
}

function connectSSE() {
  if (eventSource) {
    eventSource.close()
  }

  eventSource = new EventSource('/admin/keys/active-stream')

  eventSource.onopen = () => {
    connected.value = true
  }

  eventSource.addEventListener('snapshot', (e: MessageEvent) => {
    try {
      const data: ActiveKeyEntry[] = JSON.parse(e.data)
      activeKeys.value = data
    } catch {
      activeKeys.value = []
    }
  })

  eventSource.addEventListener('update', (e: MessageEvent) => {
    try {
      const data: ActiveKeyEntry[] = JSON.parse(e.data)
      activeKeys.value = data
    } catch {
      // 忽略解析失败
    }
  })

  eventSource.onerror = () => {
    connected.value = false
    // EventSource 会自动重连，无需手动处理
  }
}

onMounted(() => {
  connectSSE()
  // 每秒刷新持续时间显示
  durationTimer = setInterval(() => {
    tick.value++
  }, 1000)
})

onUnmounted(() => {
  if (eventSource) {
    eventSource.close()
    eventSource = null
  }
  if (durationTimer) {
    clearInterval(durationTimer)
    durationTimer = null
  }
})
</script>

<template>
  <div class="active-keys-bar">
    <div class="bar-header">
      <div class="bar-title">
        <span class="pulse-dot" :class="{ active: activeKeys.length > 0 }" aria-hidden="true"></span>
        <span>活跃密钥</span>
        <el-tag v-if="activeKeys.length > 0" size="small" type="success" round class="count-tag">
          {{ activeKeys.length }}
        </el-tag>
      </div>
      <span class="conn-status" :class="{ connected }">
        {{ connected ? '已连接' : '连接中...' }}
      </span>
    </div>

    <div class="bar-content">
      <template v-if="activeKeys.length > 0">
        <TransitionGroup name="key-card">
          <div
            v-for="entry in activeKeys"
            :key="entry.request_id"
            class="active-key-card"
          >
            <span
              class="platform-dot"
              :style="{ background: getPlatformColor(entry.platform), boxShadow: `0 0 0 3px ${getPlatformColor(entry.platform)}22` }"
              aria-hidden="true"
            ></span>
            <div class="key-info">
              <span class="key-name">{{ entry.key_name }}</span>
              <span class="key-detail">
                <span class="platform-label">{{ getPlatformLabel(entry.platform) }}</span>
                <span class="separator">·</span>
                <span class="model-label">{{ entry.model }}</span>
              </span>
            </div>
            <span class="duration">{{ formatDuration(entry.started_at) }}</span>
          </div>
        </TransitionGroup>
      </template>
      <template v-else>
        <span class="idle-text">当前无活跃请求</span>
      </template>
    </div>
  </div>
</template>

<style scoped>
.active-keys-bar {
  background: var(--ws-channel);
  border-radius: 10px;
  padding: 12px 20px;
  margin-bottom: 20px;
  border: 1px solid rgba(var(--ws-conduit-rgb), 0.08);
}

.bar-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.bar-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  color: var(--ws-conduit);
}

.pulse-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--ws-channel-rgb, #94a3b8);
  opacity: 0.5;
  transition: all 0.3s ease;
}

.pulse-dot.active {
  background: var(--ws-signal);
  opacity: 1;
  animation: pulse-glow 2s ease-in-out infinite;
}

@keyframes pulse-glow {
  0%, 100% {
    box-shadow: 0 0 0 0 rgba(var(--ws-signal-rgb), 0.4);
  }
  50% {
    box-shadow: 0 0 0 6px rgba(var(--ws-signal-rgb), 0);
  }
}

.count-tag {
  font-size: 11px;
  height: 18px;
  padding: 0 6px;
  line-height: 18px;
}

.conn-status {
  font-size: var(--text-xs);
  color: rgba(var(--ws-conduit-rgb), 0.4);
  font-family: var(--font-mono);
}

.conn-status.connected {
  color: var(--ws-signal);
}

.bar-content {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  min-height: 36px;
  align-items: center;
}

.idle-text {
  font-size: var(--text-sm);
  color: rgba(var(--ws-conduit-rgb), 0.35);
  font-style: italic;
}

.active-key-card {
  display: flex;
  align-items: center;
  gap: 8px;
  background: #fff;
  border: 1px solid rgba(var(--ws-pool-rgb), 0.15);
  border-left: 3px solid var(--ws-pool);
  border-radius: 6px;
  padding: 6px 12px;
  min-width: 200px;
  transition: all 0.3s ease;
}

.active-key-card:hover {
  border-left-color: var(--ws-signal);
  box-shadow: 0 2px 8px rgba(var(--ws-pool-rgb), 0.12);
}

.platform-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.key-info {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}

.key-name {
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
  color: var(--ws-conduit);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 160px;
}

.key-detail {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: var(--text-xs);
  color: rgba(var(--ws-conduit-rgb), 0.6);
  font-family: var(--font-mono);
}

.platform-label {
  font-family: var(--font-sans);
}

.separator {
  opacity: 0.3;
}

.model-label {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 140px;
}

.duration {
  font-size: var(--text-xs);
  color: var(--ws-pool);
  font-family: var(--font-mono);
  font-weight: var(--weight-medium);
  margin-left: auto;
  flex-shrink: 0;
}

/* TransitionGroup 动画 */
.key-card-enter-active {
  transition: all 0.3s ease;
}

.key-card-leave-active {
  transition: all 0.2s ease;
}

.key-card-enter-from {
  opacity: 0;
  transform: translateY(-8px) scale(0.96);
}

.key-card-leave-to {
  opacity: 0;
  transform: translateX(8px) scale(0.96);
}

.key-card-move {
  transition: transform 0.3s ease;
}
</style>
