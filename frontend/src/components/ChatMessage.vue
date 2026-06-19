<script setup lang="ts">
import type { DisplayMessage } from '@/types'

defineProps<{
  message: DisplayMessage
}>()
</script>

<template>
  <div :class="['chat-msg', { [message.role]: true }]">
    <!-- 系统消息 -->
    <template v-if="message.role === 'system'">
      {{ message.content }}
    </template>

    <!-- 用户消息 -->
    <template v-else-if="message.role === 'user'">
      {{ message.content }}
    </template>

    <!-- 错误消息 -->
    <template v-else-if="message.role === 'error'">
      ❌ {{ message.content }}
    </template>

    <!-- 助手消息 -->
    <template v-else-if="message.role === 'assistant'">
      <div v-if="message.thinking" class="thinking-block">
        <span class="thinking-label">💭 思考过程</span>
        {{ message.thinking }}
      </div>
      <div v-if="message.content" class="text-block">
        {{ message.content }}
      </div>
      <span v-if="message.isStreaming" class="chat-cursor">╌</span>
    </template>
  </div>
</template>

<style scoped>
.chat-msg {
  max-width: 85%;
  padding: 10px 14px;
  border-radius: var(--radius);
  font-size: var(--text-base);
  line-height: 1.6;
  word-wrap: break-word;
  white-space: pre-wrap;
}

.chat-msg.user {
  align-self: flex-end;
  background: var(--ws-pool);
  color: var(--on-pool-text);
  border-bottom-right-radius: 2px;
}

.chat-msg.assistant {
  align-self: flex-start;
  background: var(--ws-conduit);
  color: var(--on-conduit-text);
  border-bottom-left-radius: 2px;
  font-family: var(--font-mono);
  font-size: var(--text-sm);
  border-left: 3px solid var(--ws-pool);
}

.chat-msg.error {
  align-self: center;
  background: rgba(var(--ws-fault-rgb), 0.08);
  color: var(--ws-fault);
  border: 1px solid rgba(var(--ws-fault-rgb), 0.3);
  border-left: 3px solid var(--ws-fault);
  font-size: var(--text-sm);
  font-family: var(--font-mono);
  max-width: 90%;
}

.chat-msg.system {
  align-self: center;
  color: var(--text3);
  font-size: var(--text-xs);
  padding: 4px 12px;
  letter-spacing: 0.02em;
}

.thinking-block {
  background: rgba(var(--ws-pool-rgb), 0.1);
  border-left: 2px solid rgba(var(--ws-pool-rgb), 0.4);
  padding: 6px 10px;
  margin-bottom: 8px;
  border-radius: 2px;
  font-size: var(--text-xs);
  color: rgba(var(--ws-channel-rgb), 0.6);
  white-space: pre-wrap;
  word-wrap: break-word;
  font-style: italic;
}

.thinking-label {
  font-size: var(--text-xs);
  color: var(--ws-pool-light);
  font-weight: var(--weight-semibold);
  margin-bottom: 2px;
  display: block;
  font-style: normal;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.text-block {
  white-space: pre-wrap;
  word-wrap: break-word;
}

.chat-cursor {
  color: var(--ws-pool-light);
  animation: blink 1s step-end infinite;
  font-weight: bold;
}

@keyframes blink {
  50% { opacity: 0; }
}
</style>
