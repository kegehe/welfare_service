import { useState, useMemo } from 'react'
import { Card, Segmented, Table, Button, Space, Tag, Popconfirm, Tooltip, Select } from 'antd'
import { PlusOutlined, EditOutlined, DeleteOutlined, ThunderboltOutlined, SwapOutlined, SortAscendingOutlined, SortDescendingOutlined } from '@ant-design/icons'
import type { ColumnsType } from 'antd/es/table'
import type { PoolKey, KeyStatus, KeyHealthScore } from '@/types'
import { useCopyText } from '@/hooks/useCopyText'
import { getPlatformLabel, getPlatformColorHex } from '@/utils/platform'
import { HealthScoreGrid } from './HealthScoreGrid'

interface Props {
  poolKeys: PoolKey[]
  statusMap: Record<number, KeyStatus>
  healthScoreMap: Record<number, KeyHealthScore>
  testingKeyId: number | null
  onTest: (id: number) => void
  onAdd: () => void
  onEdit: (key: PoolKey) => void
  onDelete: (id: number) => void
  onToggle: (id: number) => void
}

const getStatusTag = (status: string) => {
  if (status === 'active') return <Tag color="success">活跃</Tag>
  if (status === 'disabled') return <Tag color="error">禁用</Tag>
  if (status === 'unhealthy') return <Tag color="warning">异常</Tag>
  return <Tag>{status}</Tag>
}

const getCircuitTag = (state: string) => {
  if (state === 'closed') return <Tag color="success">正常</Tag>
  if (state === 'open') return <Tag color="error">熔断</Tag>
  if (state === 'half_open') return <Tag color="warning">半开</Tag>
  return <Tag>{state}</Tag>
}

type SortBy = 'id' | 'created_at' | 'health_score'
type SortOrder = 'asc' | 'desc'

const sortOptions = [
  { label: '默认排序', value: 'id' },
  { label: '添加时间', value: 'created_at' },
  { label: '健康评分', value: 'health_score' },
]

export function PoolKeysTable({ poolKeys, statusMap, healthScoreMap, testingKeyId, onTest, onAdd, onEdit, onDelete, onToggle }: Props) {
  const [viewMode, setViewMode] = useState<'card' | 'table'>('card')
  const [sortBy, setSortBy] = useState<SortBy>('health_score')
  const [sortOrder, setSortOrder] = useState<SortOrder>('asc')
  const copyText = useCopyText()

  const sortedKeys = useMemo(() => {
    const dir = sortOrder === 'asc' ? 1 : -1
    return [...poolKeys].sort((a, b) => {
      if (sortBy === 'created_at') {
        const ta = a.created_at ? new Date(a.created_at).getTime() : 0
        const tb = b.created_at ? new Date(b.created_at).getTime() : 0
        // null values always sink to bottom
        if (ta === 0 && tb === 0) return 0
        if (ta === 0) return 1
        if (tb === 0) return -1
        return (ta - tb) * dir
      }
      if (sortBy === 'health_score') {
        const sa = healthScoreMap[a.id]?.health_score ?? -1
        const sb = healthScoreMap[b.id]?.health_score ?? -1
        // no-data values always sink to bottom
        if (sa === -1 && sb === -1) return 0
        if (sa === -1) return 1
        if (sb === -1) return -1
        return (sa - sb) * dir
      }
      // default: id
      return (a.id - b.id) * dir
    })
  }, [poolKeys, sortBy, sortOrder, healthScoreMap])

  const columns: ColumnsType<PoolKey> = [
    {
      title: 'ID',
      dataIndex: 'id',
      width: 60,
    },
    {
      title: '平台',
      dataIndex: 'platform',
      render: (v: string) => getPlatformLabel(v),
    },
    {
      title: '名称',
      dataIndex: 'name',
      ellipsis: true,
    },
    {
      title: 'Key',
      dataIndex: 'key_prefix',
      render: (v: string, record) => (
        <Tooltip title="点击复制完整 Key">
          <span
            className="key-display"
            onClick={() => copyText(record.api_key, 'Key 已复制')}
          >
            {v || '****'}
          </span>
        </Tooltip>
      ),
    },
    {
      title: '状态',
      dataIndex: 'status',
      render: (v: string) => getStatusTag(v),
    },
    {
      title: '熔断器',
      key: 'circuit',
      render: (_: any, record) => {
        const status = statusMap[record.id]
        return status ? getCircuitTag(status.circuit_state) : <Tag>未知</Tag>
      },
    },
    {
      title: '健康评分',
      key: 'health',
      width: 310,  // 100格网格: 100×2px + 99×1px gap = 299px + padding
      render: (_: any, record) => {
        const hs = healthScoreMap[record.id]
        return hs ? (
          <HealthScoreGrid score={hs.health_score} statusLabel={hs.status_label} scoreSource={hs.score_source} sampleCount={hs.sample_count} lowConfidence={hs.low_confidence} />
        ) : (
          <span className="text-muted">-</span>
        )
      },
    },
    {
      title: '限流',
      key: 'limit',
      render: (_: any, record) => {
        const tpm = record.tpm_limit || 0
        const rpm = record.rpm_limit || 0
        if (tpm === 0 && rpm === 0) return <Tag>不限</Tag>
        return <span>T:{tpm}/R:{rpm}</span>
      },
    },
    {
      title: '模型',
      dataIndex: 'models',
      ellipsis: true,
      render: (v: string[]) => v?.join(', ') || '-',
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
            title={`确认删除 Key #${record.id}？`}
            onConfirm={() => onDelete(record.id)}
            okText="确认"
            cancelText="取消"
          >
            <Tooltip title="删除">
              <Button size="small" danger icon={<DeleteOutlined />} />
            </Tooltip>
          </Popconfirm>
          <Tooltip title="测试连通性">
            <Button
              size="small"
              loading={testingKeyId === record.id}
              icon={<ThunderboltOutlined />}
              onClick={() => onTest(record.id)}
            />
          </Tooltip>
          <Tooltip title={record.status === 'active' ? '禁用' : '启用'}>
            <Button
              size="small"
              icon={<SwapOutlined />}
              onClick={() => onToggle(record.id)}
            />
          </Tooltip>
        </Space>
      ),
    },
  ]

  return (
    <Card
      title="号池管理"
      className="section-card"
      extra={
        <Space>
          <Select
            value={sortBy}
            options={sortOptions}
            onChange={(v) => setSortBy(v)}
            style={{ width: 120 }}
            size="small"
          />
          <Tooltip title={sortOrder === 'asc' ? '升序' : '降序'}>
            <Button
              size="small"
              icon={sortOrder === 'asc' ? <SortAscendingOutlined /> : <SortDescendingOutlined />}
              onClick={() => setSortOrder(sortOrder === 'asc' ? 'desc' : 'asc')}
            />
          </Tooltip>
          <Segmented
            options={[
              { label: '卡片', value: 'card' },
              { label: '表格', value: 'table' },
            ]}
            value={viewMode}
            onChange={(v) => setViewMode(v as 'card' | 'table')}
          />
          <Button type="primary" icon={<PlusOutlined />} onClick={onAdd}>
            添加 Key
          </Button>
        </Space>
      }
    >
      {viewMode === 'table' ? (
        <Table
          dataSource={sortedKeys}
          columns={columns}
          rowKey="id"
          scroll={{ x: 1400 }}
          pagination={false}
        />
      ) : (
        <div className="pool-keys-grid">
          {sortedKeys.map(key => (
            <PoolKeyCard
              key={key.id}
              poolKey={key}
              status={statusMap[key.id]}
              healthScore={healthScoreMap[key.id]}
              testing={testingKeyId === key.id}
              onEdit={() => onEdit(key)}
              onDelete={() => onDelete(key.id)}
              onToggle={() => onToggle(key.id)}
              onTest={() => onTest(key.id)}
              onCopy={() => copyText(key.api_key, 'Key 已复制')}
            />
          ))}
        </div>
      )}
    </Card>
  )
}

interface CardProps {
  poolKey: PoolKey
  status?: KeyStatus
  healthScore?: KeyHealthScore
  testing: boolean
  onEdit: () => void
  onDelete: () => void
  onToggle: () => void
  onTest: () => void
  onCopy: () => void
}

function PoolKeyCard({ poolKey, status, healthScore, testing, onEdit, onDelete, onToggle, onTest, onCopy }: CardProps) {
  return (
    <Card
      size="small"
      title={
        <div className="pool-key-card__header">
          <Tag color={getPlatformColorHex(poolKey.platform)}>{getPlatformLabel(poolKey.platform)}</Tag>
          <span className="pool-key-card__name">{poolKey.name || poolKey.key_prefix || `Key #${poolKey.id}`}</span>
          {(poolKey.name || poolKey.key_prefix) && <span className="pool-key-card__id">#{poolKey.id}</span>}
        </div>
      }
      extra={
        <Space size="small">
          <Button size="small" icon={<EditOutlined />} onClick={onEdit} />
          <Popconfirm title="确认删除?" onConfirm={onDelete}>
            <Button size="small" danger icon={<DeleteOutlined />} />
          </Popconfirm>
          <Button size="small" loading={testing} icon={<ThunderboltOutlined />} onClick={onTest} />
          <Button size="small" icon={<SwapOutlined />} onClick={onToggle} />
        </Space>
      }
    >
      <div className="pool-key-card__body">
        <div>
          <span className="pool-key-card__label">Key: </span>
          <span className="key-display" onClick={onCopy}>
            {poolKey.key_prefix || '****'}
          </span>
        </div>
        <div className="pool-key-card__tags">
          {getStatusTag(poolKey.status)}
          {status && getCircuitTag(status.circuit_state)}
        </div>
        {healthScore && (
          <HealthScoreGrid score={healthScore.health_score} statusLabel={healthScore.status_label} scoreSource={healthScore.score_source} sampleCount={healthScore.sample_count} lowConfidence={healthScore.low_confidence} />
        )}
        {poolKey.models.length > 0 && (
          <div>
            <span className="pool-key-card__label">模型: </span>
            {poolKey.models.map((m, i) => (
              <Tag key={i}>{m}</Tag>
            ))}
          </div>
        )}
        {poolKey.note && (
          <div className="pool-key-card__note">
            备注: {poolKey.note}
          </div>
        )}
      </div>
    </Card>
  )
}
