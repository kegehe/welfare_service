import { Modal, Typography, Button, Tag } from 'antd'
import { CopyOutlined } from '@ant-design/icons'
import { useCopyText } from '@/hooks/useCopyText'

const { Text } = Typography

interface Props {
  open: boolean
  newKey: string
  baseUrl: string
  availableModels: string[]
  onClose: () => void
}

export function ShowKeyModal({ open, newKey, baseUrl, availableModels, onClose }: Props) {
  const copyText = useCopyText()

  const getClaudeConfig = () => {
    return `# Claude Code 配置
export ANTHROPIC_BASE_URL="${baseUrl}"
export ANTHROPIC_API_KEY="${newKey}"`
  }

  const getCodexConfig = () => {
    return `# Codex 配置
export OPENAI_BASE_URL="${baseUrl}/v1"
export OPENAI_API_KEY="${newKey}"`
  }

  return (
    <Modal
      title="访问 Key 创建成功"
      open={open}
      onCancel={onClose}
      footer={[
        <Button key="close" type="primary" onClick={onClose}>
          关闭
        </Button>,
      ]}
      width={600}
    >
      <div className="config-section">
        <h4>您的访问 Key</h4>
        <div className="key-display-row">
          <Text code className="key-display-text">
            {newKey}
          </Text>
          <Button
            type="primary"
            icon={<CopyOutlined />}
            onClick={() => copyText(newKey, 'Key 已复制')}
          >
            复制
          </Button>
        </div>
      </div>

      <div className="config-section">
        <h4>可用模型</h4>
        <div className="config-section__title">
          {availableModels.length > 0 ? (
            availableModels.map(m => <Tag key={m}>{m}</Tag>)
          ) : (
            <Text type="secondary">暂无可用模型</Text>
          )}
        </div>
      </div>

      <div className="config-section">
        <h4>Claude Code 配置</h4>
        <pre className="config-block">{getClaudeConfig()}</pre>
        <Button
          size="small"
          icon={<CopyOutlined />}
          onClick={() => copyText(getClaudeConfig(), 'Claude 配置已复制')}
          className="config-section__copy-btn"
        >
          复制配置
        </Button>
      </div>

      <div>
        <h4>Codex 配置</h4>
        <pre className="config-block">{getCodexConfig()}</pre>
        <Button
          size="small"
          icon={<CopyOutlined />}
          onClick={() => copyText(getCodexConfig(), 'Codex 配置已复制')}
          className="config-section__copy-btn"
        >
          复制配置
        </Button>
      </div>
    </Modal>
  )
}
