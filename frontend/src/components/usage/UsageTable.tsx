import { Table, Card, Tooltip } from 'antd'
import type { ColumnsType } from 'antd/es/table'
import { COLORS } from '@/config/theme'

// 公共字段
interface BaseRow {
  id: number | string
  name: string
  requests: number
  promptTokens: number
  completionTokens: number
}

// 号池 Key 特有
interface PoolRow extends BaseRow {
  successRate: number
  avgLatencyMs: number
}

// 访问 Key 特有
interface AccessRow extends BaseRow {
  lastUsedAt: string | null
}

type TableRow = Partial<PoolRow> & BaseRow & Partial<AccessRow>

type Variant = 'pool' | 'access' | 'model'

interface Props {
  title: string
  rows: TableRow[]
  showTotal?: boolean
  variant: Variant
  size?: 'small' | 'middle'
  onRowClick?: (row: { id: number | string; name: string }) => void
}

function formatTokens(value: number): string {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1)}M`
  if (value >= 1_000) return `${(value / 1_000).toFixed(1)}K`
  return String(value)
}

function formatRequests(value: number): string {
  return value.toLocaleString()
}

function formatRate(rate: number): string {
  return `${(rate * 100).toFixed(1)}%`
}

function formatLatency(ms: number): string {
  return `${Math.round(ms)}ms`
}

function rateColor(rate: number): string {
  if (rate >= 0.98) return COLORS.signal
  if (rate >= 0.9) return COLORS.fuse
  return COLORS.fault
}

export function UsageTable({ title, rows, variant, showTotal = false, size = 'middle', onRowClick }: Props) {
  // 总 Token 列始终存在
  const baseColumns: ColumnsType<TableRow> = [
    {
      title: '名称',
      dataIndex: 'name',
      key: 'name',
      ellipsis: true,
    },
    {
      title: '请求数',
      dataIndex: 'requests',
      key: 'requests',
      align: 'right',
      sorter: (a, b) => a.requests - b.requests,
      render: (v: number) => formatRequests(v),
    },
    {
      title: '总 Tokens',
      key: 'totalTokens',
      align: 'right',
      sorter: (a, b) => (a.promptTokens + a.completionTokens) - (b.promptTokens + b.completionTokens),
      render: (_: unknown, r: TableRow) => (
        <span style={{ fontWeight: 600 }}>{formatTokens(r.promptTokens + r.completionTokens)}</span>
      ),
    },
    {
      title: '输入',
      dataIndex: 'promptTokens',
      key: 'promptTokens',
      align: 'right',
      render: (v: number) => <span style={{ color: COLORS.pool }}>{formatTokens(v)}</span>,
      sorter: (a, b) => a.promptTokens - b.promptTokens,
    },
    {
      title: '输出',
      dataIndex: 'completionTokens',
      key: 'completionTokens',
      align: 'right',
      render: (v: number) => <span style={{ color: COLORS.signal }}>{formatTokens(v)}</span>,
      sorter: (a, b) => a.completionTokens - b.completionTokens,
    },
  ]

  // 号池 Key 额外列
  const poolExtra: ColumnsType<TableRow> = variant === 'pool' ? [
    {
      title: '成功率',
      dataIndex: 'successRate',
      key: 'successRate',
      align: 'right',
      sorter: (a, b) => (a.successRate ?? 0) - (b.successRate ?? 0),
      render: (v: number) => (
        <span style={{ color: rateColor(v), fontWeight: 600 }}>{formatRate(v)}</span>
      ),
    },
    {
      title: '延迟',
      dataIndex: 'avgLatencyMs',
      key: 'avgLatencyMs',
      align: 'right',
      sorter: (a, b) => (a.avgLatencyMs ?? 0) - (b.avgLatencyMs ?? 0),
      render: (v: number) => (
        <Tooltip title={`${Math.round(v)} ms`}>
          <span>{formatLatency(v)}</span>
        </Tooltip>
      ),
    },
  ] : []

  // 访问 Key 额外列
  const accessExtra: ColumnsType<TableRow> = variant === 'access' ? [
    {
      title: '最后使用',
      dataIndex: 'lastUsedAt',
      key: 'lastUsedAt',
      align: 'right',
      render: (v: string | null) => {
        if (!v) return '-'
        try {
          return new Date(v).toLocaleString('zh-CN', {
            month: '2-digit',
            day: '2-digit',
            hour: '2-digit',
            minute: '2-digit',
          })
        } catch {
          return v
        }
      },
    },
  ] : []

  const columns = [...baseColumns, ...poolExtra, ...accessExtra]

  const showSummary = showTotal && rows.length > 0

  return (
    <Card
      title={title || undefined}
      size="small"
      styles={{ body: { padding: title ? undefined : '0 1px' } }}
    >
      <Table
        dataSource={rows}
        columns={columns}
        rowKey="id"
        pagination={false}
        size={size}
        onRow={onRowClick ? (record) => ({
          onClick: () => onRowClick({ id: record.id, name: record.name }),
          style: { cursor: 'pointer' },
        }) : undefined}
        summary={showSummary ? () => {
          const totalRequests = rows.reduce((s, r) => s + r.requests, 0)
          const totalPrompt = rows.reduce((s, r) => s + r.promptTokens, 0)
          const totalCompletion = rows.reduce((s, r) => s + r.completionTokens, 0)
          const totalTokens = totalPrompt + totalCompletion
          // 动态计算列数，确保 summary cell 数量与实际列数一致
          const columnCount = columns.length
          return (
            <Table.Summary fixed>
              <Table.Summary.Row>
                <Table.Summary.Cell index={0}>
                  <strong>合计</strong>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={1} align="right">
                  <strong>{formatRequests(totalRequests)}</strong>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={2} align="right">
                  <strong>{formatTokens(totalTokens)}</strong>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={3} align="right">
                  <strong style={{ color: COLORS.pool }}>{formatTokens(totalPrompt)}</strong>
                </Table.Summary.Cell>
                <Table.Summary.Cell index={4} align="right">
                  <strong style={{ color: COLORS.signal }}>{formatTokens(totalCompletion)}</strong>
                </Table.Summary.Cell>
                {/* 额外列的空 cell，保持合计行与表头对齐 */}
                {Array.from({ length: columnCount - 5 }, (_, i) => (
                  <Table.Summary.Cell key={i} index={5 + i} />
                ))}
              </Table.Summary.Row>
            </Table.Summary>
          )
        } : undefined}
      />
    </Card>
  )
}
