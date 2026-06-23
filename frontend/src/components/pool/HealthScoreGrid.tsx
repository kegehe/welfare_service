import { Tooltip } from 'antd'
import { WarningOutlined } from '@ant-design/icons'
import { COLORS } from '@/config/theme'

interface Props {
  score: number
  statusLabel: string
  scoreSource: string
  sampleCount?: number
  lowConfidence?: boolean
}

// 4色分级: 绿(80-100) → 黄绿(50-79) → 琥珀(20-49) → 红(0-19) → 灰(无数据)
const STATUS_COLORS: Record<string, string> = {
  normal: COLORS.signal,           // 绿色 #22C55E
  light_throttled: COLORS.lime,    // 黄绿色 #84CC16
  heavy_throttled: COLORS.fuse,    // 琥珀色 #F59E0B
  critical: COLORS.fault,          // 红色 #EF4444
}

const EMPTY_COLOR = COLORS.gridEmpty  // #e2e8f0

const statusLabelMap: Record<string, string> = {
  normal: '正常稳定',
  light_throttled: '轻度限流',
  heavy_throttled: '重度限流',
  critical: '严重异常',
  nodata: '无数据',
}

const scoreSourceMap: Record<string, string> = {
  realtime: '实时',
  window: '24h窗口',
  nodata: '无数据',
}

export function HealthScoreGrid({ score, statusLabel, scoreSource, sampleCount, lowConfidence }: Props) {
  const isNoData = statusLabel === 'nodata'
  const fillColor = isNoData ? EMPTY_COLOR : (STATUS_COLORS[statusLabel] || EMPTY_COLOR)
  const filledCount = isNoData ? 0 : Math.min(100, Math.max(0, score))
  const label = statusLabelMap[statusLabel] || statusLabel

  // score=0 且非 NoData 时（即 "有数据但全部失败"），用状态色渲染空格边框
  // 以区分 "0分信号" 和 "无数据" —— 无数据时全灰无边框，0分时灰底+红边框
  const showCriticalOutline = !isNoData && filledCount === 0 && statusLabel === 'critical'

  const sourceLabel = scoreSourceMap[scoreSource] || scoreSource
  const sampleInfo = sampleCount !== undefined ? ` | 样本: ${sampleCount}` : ''
  const confidenceInfo = lowConfidence ? ' ⚠ 低置信度' : ''

  return (
    <Tooltip title={`健康评分: ${score} - ${label} | 来源: ${sourceLabel}${sampleInfo}${confidenceInfo}`}>
      <div className="health-score-grid">
        <div className={`health-score-grid__bars${showCriticalOutline ? ' health-score-grid__bars--critical-empty' : ''}`}>
          {Array.from({ length: 100 }, (_, i) => (
            <div
              key={i}
              className={`health-score-grid__cell ${i < filledCount ? 'health-score-grid__cell--filled' : ''} ${lowConfidence && i < filledCount ? 'health-score-grid__cell--low-conf' : ''}`}
              style={{ backgroundColor: i < filledCount ? fillColor : EMPTY_COLOR }}
            />
          ))}
        </div>
        <div className={`health-score-grid__label health-score-grid__label--${statusLabel}`}>
          {score}{lowConfidence && <WarningOutlined style={{ fontSize: 10, marginLeft: 2 }} />}
        </div>
      </div>
    </Tooltip>
  )
}
