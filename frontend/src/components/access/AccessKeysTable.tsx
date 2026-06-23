import { Card, Table, Button, Space, Tag, Popconfirm, Tooltip, Collapse } from 'antd'
import { PlusOutlined, EditOutlined, DeleteOutlined, SwapOutlined, CopyOutlined } from '@ant-design/icons'
import type { ColumnsType } from 'antd/es/table'
import type { AccessKey } from '@/types'
import { useCopyText } from '@/hooks/useCopyText'

interface Props {
  accessKeys: AccessKey[]
  baseUrl: string
  availableModels: string[]
  onDelete: (id: number) => void
  onToggle: (id: number) => void
  onEdit: (key: AccessKey) => void
  onCreate: () => void
}

export function AccessKeysTable({ accessKeys, baseUrl, availableModels, onDelete, onToggle, onEdit, onCreate }: Props) {
  const copyText = useCopyText()

  const formatTime = (t: string | null) => {
    if (!t) return '-'
    return new Date(t).toLocaleString('zh-CN')
  }

  const getEffectiveUrl = () => {
    if (!baseUrl) return '未配置'
    return baseUrl
  }

  const getClaudeConfig = (key: string) => {
    const url = getEffectiveUrl()
    return `# Claude Code 配置
export ANTHROPIC_BASE_URL="${url}"
export ANTHROPIC_API_KEY="${key}"`
  }

  const getCodexConfig = (key: string) => {
    const url = getEffectiveUrl()
    return `# Codex 配置
export OPENAI_BASE_URL="${url}/v1"
export OPENAI_API_KEY="${key}"`
  }

  const columns: ColumnsType<AccessKey> = [
    {
      title: 'ID',
      dataIndex: 'id',
      width: 60,
    },
    {
      title: 'Key',
      dataIndex: 'key',
      render: (v: string) => (
        <Tooltip title="点击复制">
          <span
            className="key-display"
            onClick={() => copyText(v, 'Key 已复制')}
          >
            {v.length > 16 ? `${v.slice(0, 12)}...${v.slice(-4)}` : v}
          </span>
        </Tooltip>
      ),
    },
    {
      title: '名称',
      dataIndex: 'name',
      ellipsis: true,
    },
    {
      title: '限流',
      key: 'limit',
      render: (_: any, record) => {
        const rpm = record.rpm_limit || 0
        const tpm = record.tpm_limit || 0
        if (rpm === 0 && tpm === 0) return <Tag>不限</Tag>
        return <span>R:{rpm}/T:{tpm}</span>
      },
    },
    {
      title: '过期时间',
      dataIndex: 'expires_at',
      render: formatTime,
    },
    {
      title: '最后使用',
      dataIndex: 'last_used_at',
      render: formatTime,
    },
    {
      title: '状态',
      dataIndex: 'status',
      render: (v: string) => (
        <Tag color={v === 'active' ? 'success' : 'error'}>
          {v === 'active' ? '活跃' : '禁用'}
        </Tag>
      ),
    },
    {
      title: '操作',
      key: 'action',
      width: 200,
      render: (_: any, record) => (
        <Space>
          <Tooltip title="编辑">
            <Button size="small" icon={<EditOutlined />} onClick={() => onEdit(record)} />
          </Tooltip>
          <Popconfirm
            title={`确认删除 Access Key #${record.id}？`}
            onConfirm={() => onDelete(record.id)}
            okText="确认"
            cancelText="取消"
          >
            <Tooltip title="删除">
              <Button size="small" danger icon={<DeleteOutlined />} />
            </Tooltip>
          </Popconfirm>
          <Tooltip title={record.status === 'active' ? '禁用' : '启用'}>
            <Button size="small" icon={<SwapOutlined />} onClick={() => onToggle(record.id)} />
          </Tooltip>
        </Space>
      ),
    },
  ]

  return (
    <div className="section-card">
      {/* 接入说明 */}
      <Collapse
        className="config-section"
        items={[
          {
            key: 'instructions',
            label: '接入说明',
            children: (
              <div>
                <h4>可用模型</h4>
                <div className="config-section">
                  {availableModels.length > 0 ? (
                    availableModels.map(m => <Tag key={m}>{m}</Tag>)
                  ) : (
                    <span className="text-muted">暂无可用模型</span>
                  )}
                </div>

                <h4>Claude Code 配置</h4>
                <pre className="config-block">
                  {getClaudeConfig('<your-access-key>')}
                </pre>
                <Button
                  size="small"
                  icon={<CopyOutlined />}
                  onClick={() => copyText(getClaudeConfig('<your-access-key>'), 'Claude 配置已复制')}
                  className="config-section__copy-btn"
                >
                  复制
                </Button>

                <h4>Codex 配置</h4>
                <pre className="config-block">
                  {getCodexConfig('<your-access-key>')}
                </pre>
                <Button
                  size="small"
                  icon={<CopyOutlined />}
                  onClick={() => copyText(getCodexConfig('<your-access-key>'), 'Codex 配置已复制')}
                  className="config-section__copy-btn"
                >
                  复制
                </Button>
              </div>
            ),
          },
        ]}
      />

      {/* Key 表格 */}
      <Card
        title="访问 Key 管理"
        extra={
          <Button type="primary" icon={<PlusOutlined />} onClick={onCreate}>
            创建 Key
          </Button>
        }
      >
        <Table
          dataSource={accessKeys}
          columns={columns}
          rowKey="id"
          pagination={false}
        />
      </Card>
    </div>
  )
}
