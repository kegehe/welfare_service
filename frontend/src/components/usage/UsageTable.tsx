import { Table, Card } from 'antd'
import type { ColumnsType } from 'antd/es/table'

interface TableRow {
  id: number
  name: string
  requests: number
  promptTokens: number
  completionTokens: number
}

interface Props {
  title: string
  rows: TableRow[]
  showTotal?: boolean
}

function formatTokens(value: number): string {
  if (value >= 1000000) return `${(value / 1000000).toFixed(1)}M`
  if (value >= 1000) return `${(value / 1000).toFixed(1)}K`
  return String(value)
}

export function UsageTable({ title, rows, showTotal = false }: Props) {
  const columns: ColumnsType<TableRow> = [
    {
      title: '名称',
      dataIndex: 'name',
      key: 'name',
    },
    {
      title: '请求数',
      dataIndex: 'requests',
      key: 'requests',
      align: 'right',
      sorter: (a, b) => a.requests - b.requests,
    },
    {
      title: '输入 Tokens',
      dataIndex: 'promptTokens',
      key: 'promptTokens',
      align: 'right',
      render: (v: number) => formatTokens(v),
      sorter: (a, b) => a.promptTokens - b.promptTokens,
    },
    {
      title: '输出 Tokens',
      dataIndex: 'completionTokens',
      key: 'completionTokens',
      align: 'right',
      render: (v: number) => formatTokens(v),
      sorter: (a, b) => a.completionTokens - b.completionTokens,
    },
  ]

  const dataSource = rows

  return (
    <Card title={title} size="small">
      <Table
        dataSource={dataSource}
        columns={columns}
        rowKey="id"
        pagination={false}
        size="small"
        summary={showTotal ? () => {
          const totalRequests = rows.reduce((sum, r) => sum + r.requests, 0)
          const totalPrompt = rows.reduce((sum, r) => sum + r.promptTokens, 0)
          const totalCompletion = rows.reduce((sum, r) => sum + r.completionTokens, 0)
          return (
            <Table.Summary.Row>
              <Table.Summary.Cell index={0}>
                <strong>合计</strong>
              </Table.Summary.Cell>
              <Table.Summary.Cell index={1} align="right">
                <strong>{formatTokens(totalRequests)}</strong>
              </Table.Summary.Cell>
              <Table.Summary.Cell index={2} align="right">
                <strong>{formatTokens(totalPrompt)}</strong>
              </Table.Summary.Cell>
              <Table.Summary.Cell index={3} align="right">
                <strong>{formatTokens(totalCompletion)}</strong>
              </Table.Summary.Cell>
            </Table.Summary.Row>
          )
        } : undefined}
      />
    </Card>
  )
}
