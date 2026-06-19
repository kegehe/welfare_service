<script setup lang="ts">
import { ref, nextTick, watch, computed } from 'vue'
import { ElMessage } from 'element-plus'
import { listModels, fetchChatStream } from '@/api/chat'
import type { AccessKey, ChatProtocol, ChatHistoryMsg, DisplayMessage, StreamDelta, ModelEntry } from '@/types'
import ChatMessage from './ChatMessage.vue'

const props = defineProps<{
  accessKeys: AccessKey[]
}>()

// 对话配置
const protocol = ref<ChatProtocol>('openai')
const selectedAccessKey = ref('')
const selectedModel = ref('')
const models = ref<ModelEntry[]>([])

// 对话状态
const messages = ref<DisplayMessage[]>([
  { id: 'system-0', role: 'system', content: '选择访问 Key 和模型后即可开始对话测试', thinking: '', isStreaming: false },
])
const chatHistory = ref<ChatHistoryMsg[]>([])
const isStreaming = ref(false)
const chatAbort = ref<AbortController | null>(null)

// 输入
const chatInput = ref('')
const messagesContainer = ref<HTMLElement>()

// 活跃的访问 Key
const activeAccessKeys = computed(() => props.accessKeys.filter(k => k.status === 'active'))

// 选择 Key 后加载模型
watch(selectedAccessKey, async (key) => {
  selectedModel.value = ''
  models.value = []
  if (!key) return
  try {
    models.value = await listModels(key)
    if (models.value.length > 0) {
      selectedModel.value = models.value[0].id
    }
  } catch (e: any) {
    ElMessage.error('加载模型失败: ' + (e.message || '未知错误'))
  }
})

function scrollToBottom() {
  nextTick(() => {
    if (messagesContainer.value) {
      messagesContainer.value.scrollTop = messagesContainer.value.scrollHeight
    }
  })
}

function newConversation() {
  // 如果正在流式传输，先中止
  if (chatAbort.value) {
    chatAbort.value.abort()
    chatAbort.value = null
    isStreaming.value = false
  }
  chatHistory.value = []
  messages.value = [
    { id: 'system-new', role: 'system', content: '新对话已开始', thinking: '', isStreaming: false },
  ]
}

let msgIdCounter = 0
function nextMsgId(): string {
  return `msg-${Date.now()}-${msgIdCounter++}`
}

// SSE 增量提取
function extractDelta(data: any, proto: ChatProtocol): StreamDelta | null {
  if (!data || typeof data !== 'object') return null
  if (proto === 'openai') {
    const content = data.choices?.[0]?.delta?.content
    return content ? { text: content, type: 'text' } : null
  } else {
    if (data.type === 'content_block_delta') {
      const delta = data.delta
      if (delta?.type === 'text_delta') {
        return delta.text ? { text: delta.text, type: 'text' } : null
      }
      if (delta?.type === 'thinking_delta') {
        return delta.thinking ? { text: delta.thinking, type: 'thinking' } : null
      }
    }
    return null
  }
}

// 构造请求体
function buildRequestBody(proto: ChatProtocol, model: string, msgs: ChatHistoryMsg[]): string {
  if (proto === 'openai') {
    return JSON.stringify({ model, messages: msgs, stream: true })
  } else {
    let system: string | undefined
    const filtered = msgs.filter(m => {
      if (m.role === 'system') { system = m.content; return false }
      return true
    })
    const body: any = { model, messages: filtered, max_tokens: 16384, stream: true }
    if (system) body.system = system
    return JSON.stringify(body)
  }
}

// 读取 SSE 流
// 通过 msgIndex 在 messages 数组中定位消息，确保修改的是 Vue 响应式代理对象
async function readSSEStream(response: Response, proto: ChatProtocol, msgIndex: number) {
  const reader = response.body!.getReader()
  const decoder = new TextDecoder()
  let buffer = ''
  let textContent = ''
  let recorded = false

  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    buffer += decoder.decode(value, { stream: true })

    const parts = buffer.split('\n\n')
    buffer = parts.pop()!

    for (const part of parts) {
      const lines = part.split('\n')
      for (const line of lines) {
        if (!line.startsWith('data: ')) continue
        const payload = line.slice(6).trim()
        if (payload === '[DONE]') {
          messages.value[msgIndex].isStreaming = false
          if (textContent) chatHistory.value.push({ role: 'assistant', content: textContent })
          recorded = true
          return
        }
        try {
          const json = JSON.parse(payload)
          const delta = extractDelta(json, proto)
          if (delta) {
            const msg = messages.value[msgIndex]
            if (delta.type === 'thinking') {
              msg.thinking += delta.text
            } else {
              msg.content += delta.text
              textContent += delta.text
            }
            scrollToBottom()
          }
          if (proto === 'claude' && json.type === 'message_stop') {
            messages.value[msgIndex].isStreaming = false
            if (textContent) chatHistory.value.push({ role: 'assistant', content: textContent })
            recorded = true
            return
          }
        } catch { /* ignore parse errors */ }
      }
    }
  }

  // 流非正常结束
  messages.value[msgIndex].isStreaming = false
  if (textContent && !recorded) {
    chatHistory.value.push({ role: 'assistant', content: textContent })
  }
}

// 发送消息
async function sendChatMessage() {
  if (isStreaming.value) return

  if (!selectedAccessKey.value) {
    ElMessage.error('请先选择访问 Key')
    return
  }
  if (!selectedModel.value) {
    ElMessage.error('请先选择模型')
    return
  }

  const text = chatInput.value.trim()
  if (!text) return

  chatInput.value = ''

  // 用户消息
  messages.value.push({
    id: nextMsgId(),
    role: 'user',
    content: text,
    thinking: '',
    isStreaming: false,
  })
  chatHistory.value.push({ role: 'user', content: text })
  scrollToBottom()

  // 助手占位消息
  const assistantMsg: DisplayMessage = {
    id: nextMsgId(),
    role: 'assistant',
    content: '',
    thinking: '',
    isStreaming: true,
  }
  messages.value.push(assistantMsg)
  const assistantMsgIndex = messages.value.length - 1
  scrollToBottom()

  isStreaming.value = true
  chatAbort.value = new AbortController()

  const url = protocol.value === 'openai' ? '/v1/chat/completions' : '/v1/messages'
  const body = buildRequestBody(protocol.value, selectedModel.value, chatHistory.value)

  try {
    const res = await fetchChatStream(url, body, selectedAccessKey.value, chatAbort.value.signal)
    await readSSEStream(res, protocol.value, assistantMsgIndex)
  } catch (e: any) {
    if (e.name === 'AbortError') {
      messages.value[assistantMsgIndex].isStreaming = false
      if (messages.value[assistantMsgIndex].content) {
        chatHistory.value.push({ role: 'assistant', content: messages.value[assistantMsgIndex].content })
      }
    } else {
      // 移除助手占位，添加错误消息
      messages.value.splice(assistantMsgIndex, 1)
      chatHistory.value.pop() // 移除刚才 push 的 user message
      messages.value.push({
        id: nextMsgId(),
        role: 'error',
        content: e.message,
        thinking: '',
        isStreaming: false,
      })
    }
  } finally {
    isStreaming.value = false
    chatAbort.value = null
  }
}

function stopStreaming() {
  chatAbort.value?.abort()
}

// Enter 发送
function onInputKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    sendChatMessage()
  }
}
</script>

<template>
  <el-card shadow="hover" class="section-card">
    <template #header>
      <div class="card-header">
        <div>
          <span class="card-title">💬 对话测试</span>
          <span class="card-desc">使用访问 Key 测试 AI 对话，验证 Key 可用性</span>
        </div>
      </div>
    </template>

    <div class="chat-container">
      <!-- 配置栏 -->
      <div class="chat-config">
        <span class="config-label">协议</span>
        <el-select v-model="protocol" size="small" class="protocol-select">
          <el-option label="OpenAI" value="openai" />
          <el-option label="Claude" value="claude" />
        </el-select>

        <span class="config-label">访问 Key</span>
        <el-select v-model="selectedAccessKey" size="small" class="access-key-select" placeholder="-- 选择访问 Key --">
          <el-option
            v-for="k in activeAccessKeys"
            :key="k.id"
            :value="k.key"
            :label="(k.name || '未命名') + ' (' + k.key.substring(0, 10) + '...)'"
          />
        </el-select>

        <span class="config-label">模型</span>
        <el-select v-model="selectedModel" size="small" class="model-select" placeholder="-- 先选择 Key --">
          <el-option v-for="m in models" :key="m.id" :value="m.id" :label="m.id" />
        </el-select>

        <el-button size="small" @click="newConversation">新建对话</el-button>
      </div>

      <!-- 消息区域 -->
      <div ref="messagesContainer" class="chat-messages">
        <ChatMessage v-for="msg in messages" :key="msg.id" :message="msg" />
      </div>

      <!-- 输入区域 -->
      <div class="chat-input-area">
        <el-input
          type="textarea"
          v-model="chatInput"
          :autosize="{ minRows: 1, maxRows: 4 }"
          placeholder="输入消息，Enter 发送，Shift+Enter 换行"
          @keydown="onInputKeydown"
        />
        <el-button v-if="!isStreaming" type="primary" @click="sendChatMessage">发送</el-button>
        <el-button v-else type="danger" @click="stopStreaming">停止</el-button>
      </div>
    </div>
  </el-card>
</template>

<style scoped>
.chat-container {
  display: flex;
  flex-direction: column;
  height: 500px;
}

.chat-config {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border);
  flex-wrap: wrap;
}

.protocol-select { width: 120px; }
.access-key-select { min-width: 200px; }
.model-select { min-width: 160px; }

.config-label {
  font-size: var(--text-sm);
  color: var(--text3);
  font-weight: var(--weight-medium);
  letter-spacing: 0.03em;
}

.chat-messages {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  background: var(--surface-muted);
}

.chat-input-area {
  display: flex;
  gap: 8px;
  padding: 12px 16px;
  border-top: 1px solid var(--border);
  background: var(--card);
}
</style>
