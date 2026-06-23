import { Card, Row, Col, Statistic, Tag, Tooltip } from 'antd'
import { KeyOutlined, UserOutlined, WarningOutlined, CheckCircleOutlined } from '@ant-design/icons'
import type { PoolKey, KeyStatus, AccessKey, KeyHealthScore } from '@/types'

interface Props {
  poolKeys: PoolKey[]
  keyStatuses: KeyStatus[]
  healthScores: KeyHealthScore[]
  healthScoreError: string | null
  accessKeys: AccessKey[]
  version: string
}

export function StatsOverview({ poolKeys, keyStatuses, healthScores, healthScoreError, accessKeys, version }: Props) {
  const activePoolKeys = poolKeys.filter(k => k.status === 'active').length
  const disabledPoolKeys = poolKeys.filter(k => k.status === 'disabled').length
  const circuitOpenCount = keyStatuses.filter(s => s.circuit_state === 'open').length
  const activeAccessKeys = accessKeys.filter(k => k.status === 'active').length
  const throttledCount = healthScores.filter(s =>
    s.status_label === 'light_throttled' ||
    s.status_label === 'heavy_throttled' ||
    s.status_label === 'critical'
  ).length
  const criticalHealthCount = healthScores.filter(s =>
    s.status_label === 'heavy_throttled' ||
    s.status_label === 'critical'
  ).length
  const attentionKeyIds = new Set<number>()
  keyStatuses.forEach(s => {
    if (s.circuit_state === 'open') attentionKeyIds.add(s.key_id)
  })
  healthScores.forEach(s => {
    if (
      s.status_label === 'light_throttled' ||
      s.status_label === 'heavy_throttled' ||
      s.status_label === 'critical'
    ) {
      attentionKeyIds.add(s.key_id)
    }
  })
  const attentionCount = attentionKeyIds.size
  const attentionClass = circuitOpenCount > 0 || criticalHealthCount > 0
    ? 'circuit-alert stat-card--fault'
    : attentionCount > 0 || healthScoreError
      ? 'stat-card--info'
      : 'stat-card--signal'

  return (
    <div className="section-card">
      <div className="card-header">
        <h2 className="card-title">系统概览</h2>
        <span className="card-desc">{version}</span>
      </div>
      <Row gutter={[16, 16]}>
        <Col xs={24} sm={12} lg={6}>
          <Card className="stat-card--primary">
            <Statistic
              title="号池 Key 总数"
              value={poolKeys.length}
              prefix={<KeyOutlined />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card className="stat-card--signal">
            <Statistic
              title="活跃号池 Key"
              value={activePoolKeys}
              prefix={<CheckCircleOutlined />}
            />
            {disabledPoolKeys > 0 && (
              <Tag color="error" className="stat-tag">{disabledPoolKeys} 已禁用</Tag>
            )}
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card className="stat-card--trace">
            <Statistic
              title="访问 Key 总数"
              value={accessKeys.length}
              prefix={<UserOutlined />}
            />
            <Tag color="purple" className="stat-tag">{activeAccessKeys} 活跃</Tag>
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card className={attentionClass}>
            <Statistic
              title="健康关注"
              value={attentionCount}
              prefix={<WarningOutlined />}
            />
            {circuitOpenCount > 0 && (
              <Tag color="error" className="stat-tag">{circuitOpenCount} 熔断</Tag>
            )}
            {throttledCount > 0 && (
              <Tag color={criticalHealthCount > 0 ? 'error' : 'warning'} className="stat-tag">
                {throttledCount} 降级
              </Tag>
            )}
            {healthScoreError && (
              <Tooltip title={healthScoreError}>
                <Tag color="warning" className="stat-tag">评分未知</Tag>
              </Tooltip>
            )}
          </Card>
        </Col>
      </Row>
    </div>
  )
}
