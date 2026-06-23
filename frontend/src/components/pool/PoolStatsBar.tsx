import { useMemo } from 'react'
import { Card, Row, Col, Statistic, Tag, Tooltip } from 'antd'
import {
  DatabaseOutlined,
  CheckCircleOutlined,
  WarningOutlined,
  HeartOutlined,
  AppstoreOutlined,
  SafetyOutlined,
  LineChartOutlined,
} from '@ant-design/icons'
import type { PoolKey, KeyStatus, KeyHealthScore } from '@/types'

interface Props {
  poolKeys: PoolKey[]
  keyStatuses: KeyStatus[]
  healthScores: KeyHealthScore[]
}

export function PoolStatsBar({ poolKeys, keyStatuses, healthScores }: Props) {
  // 状态统计
  const stats = useMemo(() => {
    const total = poolKeys.length
    const activeCount = poolKeys.filter(k => k.status === 'active').length
    const disabledCount = poolKeys.filter(k => k.status === 'disabled').length
    const unhealthyCount = poolKeys.filter(k => k.status === 'unhealthy' || k.status === 'expired').length

    // 平台分布
    const platformMap: Record<string, number> = {}
    poolKeys.forEach(k => {
      platformMap[k.platform] = (platformMap[k.platform] || 0) + 1
    })

    // 熔断状态
    const circuitOpenCount = keyStatuses.filter(s => s.circuit_state === 'open').length
    const circuitHalfOpenCount = keyStatuses.filter(s => s.circuit_state === 'half_open').length

    // 健康评分分布
    const normalCount = healthScores.filter(s => s.status_label === 'normal').length
    const lightThrottledCount = healthScores.filter(s => s.status_label === 'light_throttled').length
    const heavyThrottledCount = healthScores.filter(s => s.status_label === 'heavy_throttled').length
    const criticalCount = healthScores.filter(s => s.status_label === 'critical').length
    const nodataCount = healthScores.filter(s => s.status_label === 'nodata').length
    const healthTotal = healthScores.length

    // 健康关注数（降级 + 危急）
    const degradedCount = lightThrottledCount + heavyThrottledCount + criticalCount

    // 模型覆盖
    const modelSet = new Set<string>()
    poolKeys.forEach(k => {
      if (k.status === 'active') {
        k.models.forEach(m => { if (m.trim()) modelSet.add(m.trim()) })
      }
    })

    // 有限流配置的 key 数
    const rateLimitedCount = poolKeys.filter(k =>
      (k.tpm_limit && k.tpm_limit > 0) || (k.rpm_limit && k.rpm_limit > 0)
    ).length

    // 平均成功率
    const statusesWithRate = keyStatuses.filter(s => s.success_rate > 0 || s.success_rate === 0)
    const avgSuccessRate = statusesWithRate.length > 0
      ? statusesWithRate.reduce((sum, s) => sum + s.success_rate, 0) / statusesWithRate.length
      : -1

    return {
      total, activeCount, disabledCount, unhealthyCount,
      platformMap,
      circuitOpenCount, circuitHalfOpenCount,
      normalCount, lightThrottledCount, heavyThrottledCount, criticalCount, nodataCount, healthTotal,
      degradedCount,
      modelCount: modelSet.size,
      rateLimitedCount,
      avgSuccessRate,
    }
  }, [poolKeys, keyStatuses, healthScores])

  const circuitCardClass = stats.circuitOpenCount > 0
    ? 'stat-card--fault'
    : stats.circuitHalfOpenCount > 0
      ? 'stat-card--info'
      : 'stat-card--signal'

  const healthCardClass = stats.criticalCount > 0
    ? 'stat-card--fault'
    : stats.degradedCount > 0
      ? 'stat-card--info'
      : 'stat-card--signal'

  // 平台标签颜色
  const platformColors: Record<string, string> = {
    xiaomi: 'orange',
    iflytek: 'blue',
    anthropic: 'gold',
  }

  return (
    <div className="section-card">
      <div className="card-header">
        <h2 className="card-title">号池统计</h2>
      </div>

      {/* 第一行：核心概览 */}
      <Row gutter={[16, 16]}>
        <Col xs={24} sm={12} lg={6}>
          <Card className="stat-card--primary">
            <Statistic
              title="号池总数"
              value={stats.total}
              prefix={<DatabaseOutlined />}
            />
            <div className="stat-tags">
              {Object.entries(stats.platformMap).map(([platform, count]) => (
                <Tag key={platform} color={platformColors[platform] || 'default'}>
                  {platform}: {count}
                </Tag>
              ))}
            </div>
          </Card>
        </Col>

        <Col xs={24} sm={12} lg={6}>
          <Card className="stat-card--signal">
            <Statistic
              title="活跃 / 禁用"
              value={stats.activeCount}
              prefix={<CheckCircleOutlined />}
              suffix={<span style={{ fontSize: 14, color: '#999' }}>/ {stats.total}</span>}
            />
            <div className="stat-tags">
              {stats.disabledCount > 0 && (
                <Tag color="error" className="stat-tag">{stats.disabledCount} 已禁用</Tag>
              )}
              {stats.unhealthyCount > 0 && (
                <Tag color="warning" className="stat-tag">{stats.unhealthyCount} 异常</Tag>
              )}
            </div>
          </Card>
        </Col>

        <Col xs={24} sm={12} lg={6}>
          <Card className={circuitCardClass}>
            <Statistic
              title="熔断器"
              value={stats.circuitOpenCount > 0 ? stats.circuitOpenCount : stats.circuitHalfOpenCount}
              prefix={<WarningOutlined />}
            />
            <div className="stat-tags">
              {stats.circuitOpenCount > 0 && (
                <Tag color="error" className="stat-tag">{stats.circuitOpenCount} 熔断</Tag>
              )}
              {stats.circuitHalfOpenCount > 0 && (
                <Tag color="warning" className="stat-tag">{stats.circuitHalfOpenCount} 半开</Tag>
              )}
              {stats.circuitOpenCount === 0 && stats.circuitHalfOpenCount === 0 && (
                <Tag color="success" className="stat-tag">全部正常</Tag>
              )}
            </div>
          </Card>
        </Col>

        <Col xs={24} sm={12} lg={6}>
          <Card className={healthCardClass}>
            <Statistic
              title="健康关注"
              value={stats.degradedCount}
              prefix={<HeartOutlined />}
            />
            <div className="stat-tags">
              {stats.criticalCount > 0 && (
                <Tag color="error" className="stat-tag">{stats.criticalCount} 严重</Tag>
              )}
              {stats.nodataCount > 0 && (
                <Tooltip title="健康评分数据不足的 Key">
                  <Tag className="stat-tag">{stats.nodataCount} 无数据</Tag>
                </Tooltip>
              )}
              {stats.degradedCount === 0 && stats.criticalCount === 0 && (
                <Tag color="success" className="stat-tag">全部健康</Tag>
              )}
            </div>
          </Card>
        </Col>
      </Row>

      {/* 第二行：健康评分分布 + 补充统计 */}
      <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
        <Col xs={24} lg={14}>
          <Card size="small" title="健康评分分布">
            {stats.healthTotal > 0 ? (
              <div className="health-distribution">
                <div className="health-distribution__bar">
                  {stats.normalCount > 0 && (
                    <div
                      className="health-bar-segment"
                      style={{
                        width: `${(stats.normalCount / stats.healthTotal) * 100}%`,
                        background: '#52c41a',
                      }}
                    />
                  )}
                  {stats.lightThrottledCount > 0 && (
                    <div
                      className="health-bar-segment"
                      style={{
                        width: `${(stats.lightThrottledCount / stats.healthTotal) * 100}%`,
                        background: '#fadb14',
                      }}
                    />
                  )}
                  {stats.heavyThrottledCount > 0 && (
                    <div
                      className="health-bar-segment"
                      style={{
                        width: `${(stats.heavyThrottledCount / stats.healthTotal) * 100}%`,
                        background: '#fa8c16',
                      }}
                    />
                  )}
                  {stats.criticalCount > 0 && (
                    <div
                      className="health-bar-segment"
                      style={{
                        width: `${(stats.criticalCount / stats.healthTotal) * 100}%`,
                        background: '#f5222d',
                      }}
                    />
                  )}
                  {stats.nodataCount > 0 && (
                    <div
                      className="health-bar-segment"
                      style={{
                        width: `${(stats.nodataCount / stats.healthTotal) * 100}%`,
                        background: '#d9d9d9',
                      }}
                    />
                  )}
                </div>
                <div className="health-distribution__legend">
                  <span className="health-legend-item">
                    <span className="health-dot" style={{ background: '#52c41a' }} />
                    正常 {stats.normalCount}
                    <span className="health-pct">
                      ({stats.healthTotal > 0 ? Math.round((stats.normalCount / stats.healthTotal) * 100) : 0}%)
                    </span>
                  </span>
                  <span className="health-legend-item">
                    <span className="health-dot" style={{ background: '#f59e0b' }} />
                    轻度降级 {stats.lightThrottledCount}
                    <span className="health-pct">
                      ({stats.healthTotal > 0 ? Math.round((stats.lightThrottledCount / stats.healthTotal) * 100) : 0}%)
                    </span>
                  </span>
                  <span className="health-legend-item">
                    <span className="health-dot" style={{ background: '#f97316' }} />
                    重度降级 {stats.heavyThrottledCount}
                    <span className="health-pct">
                      ({stats.healthTotal > 0 ? Math.round((stats.heavyThrottledCount / stats.healthTotal) * 100) : 0}%)
                    </span>
                  </span>
                  <span className="health-legend-item">
                    <span className="health-dot" style={{ background: '#ef4444' }} />
                    危急 {stats.criticalCount}
                    <span className="health-pct">
                      ({stats.healthTotal > 0 ? Math.round((stats.criticalCount / stats.healthTotal) * 100) : 0}%)
                    </span>
                  </span>
                  {stats.nodataCount > 0 && (
                    <span className="health-legend-item">
                      <span className="health-dot" style={{ background: '#d9d9d9' }} />
                      无数据 {stats.nodataCount}
                    </span>
                  )}
                </div>
              </div>
            ) : (
              <div style={{ color: '#999', textAlign: 'center', padding: '12px 0' }}>暂无数据</div>
            )}
          </Card>
        </Col>

        <Col xs={24} lg={10}>
          <Card size="small" title="补充统计">
            <div className="pool-supplement-stats">
              <div className="supplement-item">
                <AppstoreOutlined style={{ color: '#06b6d4', marginRight: 8 }} />
                <span className="supplement-label">支持模型</span>
                <span className="supplement-value">{stats.modelCount} 个</span>
              </div>
              <div className="supplement-item">
                <SafetyOutlined style={{ color: '#8b5cf6', marginRight: 8 }} />
                <span className="supplement-label">有限流配置</span>
                <span className="supplement-value">{stats.rateLimitedCount} 个</span>
              </div>
              <div className="supplement-item">
                <LineChartOutlined style={{ color: '#22c55e', marginRight: 8 }} />
                <span className="supplement-label">平均成功率</span>
                <span className="supplement-value">
                  {stats.avgSuccessRate >= 0
                    ? `${(stats.avgSuccessRate * 100).toFixed(1)}%`
                    : '-'}
                </span>
              </div>
            </div>
          </Card>
        </Col>
      </Row>
    </div>
  )
}
