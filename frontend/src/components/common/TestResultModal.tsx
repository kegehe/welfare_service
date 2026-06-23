import { Modal, Descriptions, Tag, Typography, Alert } from 'antd'
import type { TestKeyResult } from '@/types'

const { Text } = Typography

interface Props {
  open: boolean
  result: TestKeyResult | null
  onClose: () => void
}

interface EndpointResult {
  success?: boolean
  reachable?: boolean
  key_valid?: boolean
  upstream_error?: boolean
  latency_ms?: number
  status?: number
  error?: string
}

const getStatusTag = (data?: EndpointResult | null) => {
  if (!data) return <Tag>未测试</Tag>
  if (data.upstream_error) return <Tag color="warning">上游故障 ({data.status})</Tag>
  if (data.status === 429) return <Tag color="warning">限流 (429)</Tag>
  if (data.reachable === false) return <Tag color="error">不可达</Tag>
  if (data.key_valid === false) return <Tag color="warning">Key 无效 ({data.status})</Tag>
  if (data.success) return <Tag color="success">正常</Tag>
  if (data.reachable && data.key_valid) return <Tag color="success">正常</Tag>
  return <Tag>未知</Tag>
}

export function TestResultModal({ open, result, onClose }: Props) {
  if (!result) return null

  const renderEndpoint = (name: string, data?: EndpointResult | null, isClaude = false) => {
    if (!data) return null

    return (
      <Descriptions.Item label={name}>
        <div className="test-endpoint-info">
          <div>{getStatusTag(data)}</div>
          {data.latency_ms !== undefined && (
            <Text type="secondary">延迟: {data.latency_ms}ms</Text>
          )}
          {data.status !== undefined && (
            <Text type="secondary">HTTP: {data.status}</Text>
          )}
          {data.error && (
            <Text type="danger" className="test-endpoint-error">{data.error}</Text>
          )}
          {isClaude && data.status === 405 && (
            <Alert
              type="info"
              message="Claude API 不支持 GET 请求，返回 405 即代表端点可达、Key 有效"
              className="test-endpoint-alert"
              showIcon
            />
          )}
        </div>
      </Descriptions.Item>
    )
  }

  return (
    <Modal
      title="连通性测试结果"
      open={open}
      onCancel={onClose}
      footer={null}
      width={500}
    >
      <Descriptions column={1} bordered size="small">
        {renderEndpoint('OpenAI 端点', result.openai)}
        {renderEndpoint('Claude 端点', result.claude, true)}
      </Descriptions>

      <div className="test-result-summary">
        <Tag
          color={result.available ? 'success' : 'error'}
          className="test-result-tag"
        >
          {result.available ? '✅ 测试通过' : '❌ 测试失败'}
        </Tag>
      </div>
    </Modal>
  )
}
