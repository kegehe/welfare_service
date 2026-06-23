import { useState, useEffect } from 'react'
import { Card, Segmented, Spin, Row, Col, Statistic, message } from 'antd'
import { getStatsOverview, getPoolKeyStats, getAccessKeyStats } from '@/api/admin'
import type { StatsOverview as StatsOverviewType, PoolKeyStats, AccessKeyStats } from '@/types'
import { UsageTable } from './UsageTable'
import { UsageChart } from './UsageChart'

export function UsageStats() {
  const [hours, setHours] = useState<24 | 168 | 720>(24)
  const [overview, setOverview] = useState<StatsOverviewType | null>(null)
  const [poolStats, setPoolStats] = useState<PoolKeyStats[]>([])
  const [accessStats, setAccessStats] = useState<AccessKeyStats[]>([])
  const [loading, setLoading] = useState(false)

  useEffect(() => {
    const loadData = async () => {
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
      } catch (e: any) {
        message.error('加载统计数据失败')
      } finally {
        setLoading(false)
      }
    }
    loadData()
  }, [hours])

  const poolTableRows = poolStats.map(s => ({
    id: s.key_id,
    name: s.name || `Key #${s.key_id}`,
    requests: s.total_requests,
    promptTokens: s.total_prompt_tokens,
    completionTokens: s.total_completion_tokens,
  }))

  const accessTableRows = accessStats.map(s => ({
    id: s.access_key_id,
    name: s.name || `Access #${s.access_key_id}`,
    requests: s.total_requests,
    promptTokens: s.total_prompt_tokens,
    completionTokens: s.total_completion_tokens,
  }))

  return (
    <Spin spinning={loading}>
      <div className="section-card">
        <div className="card-header">
          <h2 className="card-title">用量统计</h2>
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

        {overview && (
          <Row gutter={[16, 16]} className="section-card__row">
            <Col xs={24} sm={8}>
              <Card>
                <Statistic title="总请求数" value={overview.total_requests} />
              </Card>
            </Col>
            <Col xs={24} sm={8}>
              <Card>
                <Statistic title="活跃号池 Key" value={overview.active_pool_keys} />
              </Card>
            </Col>
            <Col xs={24} sm={8}>
              <Card>
                <Statistic title="活跃访问 Key" value={overview.active_access_keys} />
              </Card>
            </Col>
          </Row>
        )}

        <Row gutter={[16, 16]}>
          <Col xs={24} lg={12}>
            <UsageTable
              title="号池 Key 用量"
              rows={poolTableRows}
              showTotal={false}
            />
          </Col>
          <Col xs={24} lg={12}>
            <UsageTable
              title="访问 Key 用量"
              rows={accessTableRows}
              showTotal={true}
            />
          </Col>
        </Row>

        <Card title="Token 用量趋势" className="section-card__spaced">
          <UsageChart hours={hours} />
        </Card>
      </div>
    </Spin>
  )
}
