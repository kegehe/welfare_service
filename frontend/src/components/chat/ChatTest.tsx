import { useState, useRef, useEffect, useCallback } from 'react'
import { Card, Select, Button, Input, Space, message } from 'antd'
import { SendOutlined, StopOutlined, PlusOutlined } from '@ant-design/icons'
import type { AccessKey, ChatProtocol, ChatHistoryMsg, DisplayMessage, StreamDelta } from '@/types'
import { listModels, fetchChatStream } from '@/api/chat'
import { ChatMessage } from './ChatMessage'

const { TextArea } = Input

interface Props {
  accessKeys: AccessKey[]
}

let msgIdCounter = 0
function nextMsgId(): string {
  return `msg-${Date.now()}-${msgIdCounter++}`
}

/** SSE 增量提取 */
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

/** 构造请求体（Claude 协议需要提取 system 消息到顶层，并设置 max_tokens） */
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

export function ChatTest({ accessKeys }: Props) {
  const [protocol, setProtocol] = useState<ChatProtocol>('openai')
  const [selectedAccessKey, setSelectedAccessKey] = useState('')
  const [models, setModels] = useState<string[]>([])
  const [selectedModel, setSelectedModel] = useState<string>('')
  const [messages, setMessages] = useState<DisplayMessage[]>([
    { id: 'system-0', role: 'system', content: '选择访问 Key 和模型后即可开始对话测试', thinking: '', isStreaming: false },
  ])
  const [chatHistory, setChatHistory] = useState<ChatHistoryMsg[]>([])
  const [input, setInput] = useState('')
  const [streaming, setStreaming] = useState(false)
  const [loadingModels, setLoadingModels] = useState(false)
  const abortControllerRef = useRef<AbortController | null>(null)
  const messagesEndRef = useRef<HTMLDivElement>(null)
  const messagesRef = useRef<DisplayMessage[]>(messages)
  const chatHistoryRef = useRef<ChatHistoryMsg[]>(chatHistory)

  // 保持 ref 与 state 同步（供 SSE 闭包使用）
  useEffect(() => { messagesRef.current = messages }, [messages])
  useEffect(() => { chatHistoryRef.current = chatHistory }, [chatHistory])

  // 加载模型列表
  useEffect(() => {
    if (!selectedAccessKey) {
      setModels([])
      setSelectedModel('')
      return
    }

    setLoadingModels(true)
    listModels(selectedAccessKey)
      .then(res => {
        const modelIds = res?.map(m => m.id) || []
        setModels(modelIds)
        if (modelIds.length > 0) {
          setSelectedModel(modelIds[0])
        }
      })
      .catch(e => {
        console.error('加载模型失败:', e)
        message.error('加载模型列表失败')
      })
      .finally(() => {
        setLoadingModels(false)
      })
  }, [selectedAccessKey])

  // 自动滚动到底部
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  const handleSend = useCallback(async () => {
    if (streaming) return
    if (!selectedAccessKey) {
      message.error('请先选择访问 Key')
      return
    }
    if (!selectedModel) {
      message.error('请先选择模型')
      return
    }

    const text = input.trim()
    if (!text) return

    setInput('')

    // 用户消息
    const userMsg: DisplayMessage = {
      id: nextMsgId(),
      role: 'user',
      content: text,
      thinking: '',
      isStreaming: false,
    }
    const userHistory: ChatHistoryMsg = { role: 'user', content: text }
    setMessages(prev => [...prev, userMsg])
    setChatHistory(prev => [...prev, userHistory])

    // 助手占位消息
    const assistantMsgId = nextMsgId()
    const assistantMsg: DisplayMessage = {
      id: assistantMsgId,
      role: 'assistant',
      content: '',
      thinking: '',
      isStreaming: true,
    }
    setMessages(prev => [...prev, assistantMsg])
    setStreaming(true)

    const controller = new AbortController()
    abortControllerRef.current = controller

    const url = protocol === 'openai' ? '/v1/chat/completions' : '/v1/messages'
    const currentHistory = [...chatHistoryRef.current, userHistory]
    const body = buildRequestBody(protocol, selectedModel, currentHistory)

    // 在 try 外定义，供 catch 中 AbortError 使用
    let textContent = ''
    let thinkingContent = ''

    try {
      const response = await fetchChatStream(url, body, selectedAccessKey, controller.signal)

      const reader = response.body?.getReader()
      if (!reader) throw new Error('无法读取响应流')

      const decoder = new TextDecoder()
      let buffer = ''
      let recorded = false

      while (true) {
        const { done, value } = await reader.read()
        if (done) break
        buffer += decoder.decode(value, { stream: true })

        // 按 \n\n 分割 SSE 事件（缓冲区机制）
        const parts = buffer.split('\n\n')
        buffer = parts.pop()!

        for (const part of parts) {
          if (recorded) break // 外层循环也检查，确保 [DONE]/message_stop 后停止
          const lines = part.split('\n')
          for (const line of lines) {
            if (!line.startsWith('data: ')) continue
            const payload = line.slice(6).trim()
            if (payload === '[DONE]') {
              // 流结束
              setMessages(prev => prev.map(m =>
                m.id === assistantMsgId ? { ...m, isStreaming: false } : m
              ))
              if (textContent) {
                setChatHistory(prev => [...prev, { role: 'assistant', content: textContent }])
              }
              recorded = true
              break
            }

            try {
              const json = JSON.parse(payload)
              const delta = extractDelta(json, protocol)
              if (delta) {
                if (delta.type === 'thinking') {
                  thinkingContent += delta.text
                } else {
                  textContent += delta.text
                }
                // 更新消息
                setMessages(prev => prev.map(m =>
                  m.id === assistantMsgId
                    ? { ...m, content: textContent, thinking: thinkingContent }
                    : m
                ))
              }
              // Claude 协议的 message_stop 事件
              if (protocol === 'claude' && json.type === 'message_stop') {
                setMessages(prev => prev.map(m =>
                  m.id === assistantMsgId ? { ...m, isStreaming: false } : m
                ))
                if (textContent) {
                  setChatHistory(prev => [...prev, { role: 'assistant', content: textContent }])
                }
                recorded = true
                break
              }
            } catch {
              // 忽略解析错误
            }
          }
        }
      }

      // 流非正常结束
      if (!recorded) {
        setMessages(prev => prev.map(m =>
          m.id === assistantMsgId ? { ...m, isStreaming: false } : m
        ))
        if (textContent) {
          setChatHistory(prev => [...prev, { role: 'assistant', content: textContent }])
        }
      }
    } catch (e: any) {
      if (e.name === 'AbortError') {
        // 用户中止：保留已接收的内容（使用闭包变量 textContent，避免 ref 滞后）
        setMessages(prev => prev.map(m =>
          m.id === assistantMsgId ? { ...m, isStreaming: false } : m
        ))
        if (textContent) {
          setChatHistory(prev => [...prev, { role: 'assistant', content: textContent }])
        }
      } else {
        // 发送失败：移除助手占位，添加错误消息，移除刚添加的 user history
        setMessages(prev => {
          const without = prev.filter(m => m.id !== assistantMsgId)
          return [...without, {
            id: nextMsgId(),
            role: 'error' as const,
            content: e.message || '发送失败',
            thinking: '',
            isStreaming: false,
          }]
        })
        setChatHistory(prev => prev.slice(0, -1)) // 移除 user 消息
      }
    } finally {
      setStreaming(false)
      abortControllerRef.current = null
    }
  }, [input, selectedModel, protocol, selectedAccessKey, streaming])

  const handleStop = useCallback(() => {
    abortControllerRef.current?.abort()
    setStreaming(false)
  }, [])

  const handleNewChat = useCallback(() => {
    if (abortControllerRef.current) {
      abortControllerRef.current.abort()
      abortControllerRef.current = null
    }
    setStreaming(false)
    setChatHistory([])
    setMessages([
      { id: 'system-new', role: 'system', content: '新对话已开始', thinking: '', isStreaming: false },
    ])
  }, [])

  const activeAccessKeys = accessKeys.filter(k => k.status === 'active')

  return (
    <Card title="对话测试" className="section-card">
      <Space direction="vertical" style={{ width: '100%' }} size="middle">
        {/* 配置栏 */}
        <Space wrap>
          <Select
            value={protocol}
            onChange={setProtocol}
            options={[
              { label: 'OpenAI', value: 'openai' },
              { label: 'Claude', value: 'claude' },
            ]}
            style={{ width: 120 }}
            disabled={streaming}
          />
          <Select
            placeholder="选择访问 Key"
            value={selectedAccessKey || undefined}
            onChange={setSelectedAccessKey}
            options={activeAccessKeys.map(k => ({
              label: k.name || k.key.slice(0, 12) + '...',
              value: k.key,
            }))}
            style={{ width: 200 }}
            disabled={streaming}
          />
          <Select
            placeholder="选择模型"
            value={selectedModel || undefined}
            onChange={setSelectedModel}
            options={models.map(m => ({ label: m, value: m }))}
            style={{ width: 200 }}
            loading={loadingModels}
            disabled={streaming || !selectedAccessKey}
          />
          <Button icon={<PlusOutlined />} onClick={handleNewChat} disabled={streaming}>
            新对话
          </Button>
        </Space>

        {/* 消息列表 */}
        <div className="chat-container">
          {messages.map(msg => (
            <ChatMessage key={msg.id} message={msg} />
          ))}
          <div ref={messagesEndRef} />
        </div>

        {/* 输入框 */}
        <Space.Compact style={{ width: '100%' }}>
          <TextArea
            value={input}
            onChange={e => setInput(e.target.value)}
            placeholder="输入消息... (Enter 发送, Shift+Enter 换行)"
            autoSize={{ minRows: 1, maxRows: 4 }}
            disabled={streaming || !selectedAccessKey || !selectedModel}
            onPressEnter={e => {
              if (!e.shiftKey) {
                e.preventDefault()
                handleSend()
              }
            }}
            style={{ flex: 1 }}
          />
          {streaming ? (
            <Button
              danger
              icon={<StopOutlined />}
              onClick={handleStop}
              style={{ height: 'auto' }}
            >
              停止
            </Button>
          ) : (
            <Button
              type="primary"
              icon={<SendOutlined />}
              onClick={handleSend}
              disabled={!input.trim() || !selectedAccessKey || !selectedModel}
              style={{ height: 'auto' }}
            >
              发送
            </Button>
          )}
        </Space.Compact>
      </Space>
    </Card>
  )
}
