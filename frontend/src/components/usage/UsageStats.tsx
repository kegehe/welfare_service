import { useState, useEffect, useCallback, useRef } from 'react'
import { Card, Segmented, Spin, Row, Col, Statistic, message, Drawer } from 'antd'
import { ReloadOutlined } from '@ant-design/icons'
import {
  getStatsOverview, getPoolKeyStats, getAccessKeyStats,
  getPoolKeyStatsDetail, getAccessKeyStatsDetail,
} from '@/api/admin'
import type {
  StatsOverview as StatsOverviewType, PoolKeyStats, AccessKeyStats,
  PoolKeyStatsDetail, AccessKeyStatsDetail, ModelStats,
} from '@/types'
import { UsageTable } from './UsageTable'
import { UsageChart } from './UsageChart'
import { RequestChart } from './RequestChart'
import { ModelDonut } from './ModelDonut'

const AUTO_REFRESH_MS = 30_000

export function UsageStats() {
  const [hours, setHours] = useState<24 | 168 | 720>(24)
  const [overview, setOverview] = useState<StatsOverviewType | null>(null)
  const [poolStats, setPoolStats] = useState<PoolKeyStats[]>([])
  const [accessStats, setAccessStats] = useState<AccessKeyStats[]>([])
  const [loading, setLoading] = useState(false)

  // 详情抽屉
  const [detailOpen, setDetailOpen] = useState(false)
  const [detailLoading, setDetailLoading] = useState(false)
  const [detailTitle, setDetailTitle] = useState('')
  const [detailModels, setDetailModels] = useState<ModelStats[]>([])
  const [detailSummary, setDetailSummary] = useState<Record<string, number | string | null>>({})

  const hoursRef = useRef(hours)
  hoursRef.current = hours

  const loadData = useCallback(async () => {
    setLoading(true)
    try {
      const [overviewData, poolData, accessData] = await Promise.all([
        getStatsOverview(hours),
        getPoolKeyStats(hours),
        getAccessKeyStats(hours),
      ])
      setOverview(overviewData)
      setPoolStats(poolData.keys || [])
      setAccessStats(accessData.keys || [])
    } catch {
      message.error('加载统计数据失败')
    } finally {
      setLoading(false)
    }
  }, [hours])

  useEffect(() => {
    loadData()
  }, [loadData])

  // 自动刷新
  useEffect(() => {
    const id = setInterval(() => {
      // 只在非详情弹窗打开时自动刷新
      if (!detailOpen) loadData()
    }, AUTO_REFRESH_MS)
    return () => clearInterval(id)
  }, [loadData, detailOpen])

  const handlePoolRowClick = useCallback(async (row: { id: number | string; name: string }) => {
    setDetailOpen(true)
    setDetailLoading(true)
    setDetailTitle(`号池 Key 详情 — ${row.name}`)
    setDetailModels([])
    setDetailSummary({})
    try {
      const data: PoolKeyStatsDetail = await getPoolKeyStatsDetail(row.id as number, hoursRef.current)
      setDetailModels(data.by_model || [])
      setDetailSummary({
        '总请求数': data.total_requests,
        '输入 Tokens': data.total_prompt_tokens,
        '输出 Tokens': data.total_completion_tokens,
        '成功率': data.success_rate != null ? `${(data.success_rate * 100).toFixed(1)}%` : '-',
        '平均延迟': data.avg_latency_ms != null ? `${Math.round(data.avg_latency_ms)}ms` : '-',
        '平台': data.platform || '-',
      })
    } catch {
      message.error('加载详情失败')
    } finally {
      setDetailLoading(false)
    }
  }, [])

  const handleAccessRowClick = useCallback(async (row: { id: number | string; name: string }) => {
    setDetailOpen(true)
    setDetailLoading(true)
    setDetailTitle(`访问 Key 详情 — ${row.name}`)
    setDetailModels([])
    setDetailSummary({})
    try {
      const data: AccessKeyStatsDetail = await getAccessKeyStatsDetail(row.id as number, hoursRef.current)
      setDetailModels(data.by_model || [])
      setDetailSummary({
        '总请求数': data.total_requests,
        '输入 Tokens': data.total_prompt_tokens,
        '输出 Tokens': data.total_completion_tokens,
        '最后使用': data.last_used_at || '-',
      })
    } catch {
      message.error('加载详情失败')
    } finally {
      setDetailLoading(false)
    }
  }, [])

  const poolTableRows = poolStats
    .filter(s => s.total_requests > 0)
    .map(s => ({
      id: s.key_id,
      name: s.name || `Key #${s.key_id}`,
      requests: s.total_requests,
      promptTokens: s.total_prompt_tokens,
      completionTokens: s.total_completion_tokens,
      successRate: s.success_rate,
      avgLatencyMs: s.avg_latency_ms,
    }))

  const accessTableRows = accessStats
    .filter(s => s.total_requests > 0)
    .map(s => ({
      id: s.access_key_id,
      name: s.name || `Access #${s.access_key_id}`,
      requests: s.total_requests,
      promptTokens: s.total_prompt_tokens,
      completionTokens: s.total_completion_tokens,
      lastUsedAt: s.last_used_at,
    }))

  const poolKeys = poolStats.filter(s => s.total_requests > 0).map(s => ({ key_id: s.key_id, name: s.name || `Key #${s.key_id}` }))
  const accessKeys = accessStats.filter(s => s.total_requests > 0).map(s => ({ access_key_id: s.access_key_id, name: s.name || `Access #${s.access_key_id}` }))

  return (
    <Spin spinning={loading}>
      <div className="section-card">
        <div className="card-header">
          <h2 className="card-title">用量统计</h2>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <ReloadOutlined
              style={{ cursor: 'pointer', color: '#94a3b8' }}
              onClick={loadData}
              spin={loading}
            />
            <Segmented
              options={[
                { label: '24 小时', value: 24 },
                { label: '7 天', value: 168 },
                { label: '30 天', value: 720 },
              ]}
              value={hours}
              onChange={(v) => setHours(v as 24 | 168 | 720)}
            />
          </div>
        </div>

        {/* ── KPI 概览（6 栏同排）── */}
        {overview && (
          <Row gutter={[16, 16]} className="section-card__row">
            <Col xs={12} sm={4}>
              <Card>
                <Statistic title="总请求数" value={overview.total_requests} />
                <div style={{ fontSize: 11, color: '#94a3b8', marginTop: 4 }}>
                  均速 {overview.total_requests > 0 ? (overview.total_requests / hours).toFixed(1) : '0'} req/h
                </div>
              </Card>
            </Col>
            <Col xs={12} sm={4}>
              <Card>
                <Statistic
                  title="总计 Tokens"
                  value={overview.total_tokens}
                />
              </Card>
            </Col>
            <Col xs={12} sm={4}>
              <Card>
                <Statistic
                  title="输入 Tokens"
                  value={overview.total_prompt_tokens}
                  valueStyle={{ color: '#06b6d4' }}
                />
              </Card>
            </Col>
            <Col xs={12} sm={4}>
              <Card>
                <Statistic
                  title="输出 Tokens"
                  value={overview.total_completion_tokens}
                  valueStyle={{ color: '#22c55e' }}
                />
              </Card>
            </Col>
            <Col xs={12} sm={4}>
              <Card>
                <Statistic title="活跃号池 Key" value={overview.active_pool_keys} />
              </Card>
            </Col>
            <Col xs={12} sm={4}>
              <Card>
                <Statistic title="活跃访问 Key" value={overview.active_access_keys} />
              </Card>
            </Col>
          </Row>
        )}

        {/* ── 图表区（Token 趋势 + 请求量趋势 并排）── */}
        <Row gutter={[16, 16]} className="section-card__spaced">
          <Col xs={24} lg={12}>
            <Card title="Token 用量趋势" styles={{ body: { padding: '12px 16px' } }}>
              <UsageChart
                hours={hours}
                poolKeys={poolKeys}
                accessKeys={accessKeys}
              />
            </Card>
          </Col>
          <Col xs={24} lg={12}>
            <Card title="请求量趋势" styles={{ body: { padding: '12px 16px' } }}>
              <RequestChart
                hours={hours}
                poolKeys={poolKeys}
                accessKeys={accessKeys}
              />
            </Card>
          </Col>
        </Row>

        {/* ── 模型占比 + 表格区 ── */}
        <Row gutter={[16, 16]} className="section-card__spaced">
          <Col xs={24} lg={8}>
            <Card title="模型用量占比" styles={{ body: { padding: '12px 16px' } }}>
              <ModelDonut hours={hours} />
            </Card>
          </Col>
          <Col xs={24} lg={16}>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
              <UsageTable
                title="号池 Key 用量"
                rows={poolTableRows}
                variant="pool"
                onRowClick={handlePoolRowClick}
              />
              <UsageTable
                title="访问 Key 用量"
                rows={accessTableRows}
                variant="access"
                showTotal
                onRowClick={handleAccessRowClick}
              />
            </div>
          </Col>
        </Row>

        {/* ── 详情抽屉 ── */}
        <Drawer
          title={detailTitle}
          open={detailOpen}
          onClose={() => setDetailOpen(false)}
          width={520}
          destroyOnClose
        >
          {detailLoading ? (
            <Spin style={{ display: 'block', marginTop: 40, textAlign: 'center' }} />
          ) : (
            <>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '8px 16px', marginBottom: 16 }}>
                {Object.entries(detailSummary).map(([label, value]) => (
                  <div key={label}>
                    <div style={{ fontSize: 12, color: '#94a3b8' }}>{label}</div>
                    <div style={{ fontSize: 16, fontWeight: 600, fontFamily: 'var(--font-mono)' }}>
                      {typeof value === 'number' ? value.toLocaleString() : (value ?? '-')}
                    </div>
                  </div>
                ))}
              </div>
              {detailModels.length > 0 && (
                <Card title="按模型细分" size="small">
                  <UsageTable
                    title=""
                    rows={detailModels.map(m => ({
                      id: m.model,
                      name: m.model,
                      requests: m.requests,
                      promptTokens: m.prompt_tokens,
                      completionTokens: m.completion_tokens,
                    }))}
                    variant="model"
                    size="small"
                  />
                </Card>
              )}
            </>
          )}
        </Drawer>
      </div>
    </Spin>
  )
}
