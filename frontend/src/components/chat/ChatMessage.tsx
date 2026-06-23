import { Tag } from 'antd'
import { UserOutlined, RobotOutlined, InfoCircleOutlined, WarningOutlined } from '@ant-design/icons'
import type { DisplayMessage } from '@/types'

interface Props {
  message: DisplayMessage
}

export function ChatMessage({ message }: Props) {
  const isUser = message.role === 'user'
  const isAssistant = message.role === 'assistant'
  const isSystem = message.role === 'system'
  const isError = message.role === 'error'

  const getIcon = () => {
    if (isUser) return <UserOutlined />
    if (isAssistant) return <RobotOutlined />
    if (isSystem) return <InfoCircleOutlined />
    if (isError) return <WarningOutlined />
    return null
  }

  const getTagColor = () => {
    if (isUser) return 'cyan'
    if (isAssistant) return 'purple'
    if (isSystem) return 'blue'
    if (isError) return 'red'
    return 'default'
  }

  const getLabel = () => {
    if (isUser) return '用户'
    if (isAssistant) return '助手'
    if (isSystem) return '系统'
    if (isError) return '错误'
    return ''
  }

  // Exhaustive role-to-CSS-class mapping with fallback
  const roleClass: Record<DisplayMessage['role'], string> = {
    user: 'user',
    assistant: 'assistant',
    system: 'system',
    error: 'error',
  }
  const cssRole = roleClass[message.role] ?? 'assistant'

  return (
    <div className={`chat-message${isUser ? ' chat-message--user' : ''}`}>
      <div className={`chat-message__avatar chat-message__avatar--${cssRole}`}>
        {getIcon()}
      </div>
      <div className={`chat-message__bubble chat-message__bubble--${cssRole}`}>
        <div className="chat-message__label">
          <Tag color={getTagColor()} style={{ margin: 0 }}>
            {getLabel()}
          </Tag>
        </div>

        {/* 思考过程（Claude 协议特有） */}
        {message.thinking && (
          <div className="chat-message__thinking">
            <div className="chat-message__thinking-title">💭 思考过程：</div>
            <pre className="chat-message__thinking-content">
              {message.thinking}
            </pre>
          </div>
        )}

        {/* 消息内容 */}
        <pre className="chat-message__content">
          {message.content || (isAssistant ? '...' : '')}
        </pre>
      </div>
    </div>
  )
}
