import { useState, useEffect, useRef, useCallback } from 'react'
import { Card, Tag, Empty, Badge } from 'antd'
import { WifiOutlined, DisconnectOutlined } from '@ant-design/icons'
import { getPlatformLabel, getPlatformColorHex } from '@/utils/platform'
import type { ActiveKeyEntry } from '@/types'

export function ActiveKeysBar() {
  const [entries, setEntries] = useState<ActiveKeyEntry[]>([])
  const [connected, setConnected] = useState(false)
  const [tick, setTick] = useState(0)
  const eventSourceRef = useRef<EventSource | null>(null)

  // 每秒刷新持续时间显示
  useEffect(() => {
    const timer = setInterval(() => setTick(t => t + 1), 1000)
    return () => clearInterval(timer)
  }, [])

  useEffect(() => {
    const es = new EventSource('/admin/keys/active-stream')
    eventSourceRef.current = es

    es.addEventListener('snapshot', (e) => {
      try {
        setEntries(JSON.parse(e.data))
        setConnected(true)
      } catch {
        // ignore parse error
      }
    })

    es.addEventListener('update', (e) => {
      try {
        const data = JSON.parse(e.data)
        // 服务端 update 事件返回完整列表，直接替换
        if (Array.isArray(data)) {
          setEntries(data)
        }
      } catch {
        // ignore parse error
      }
    })

    es.onerror = () => {
      setConnected(false)
    }

    es.onopen = () => {
      setConnected(true)
    }

    return () => {
      es.close()
      eventSourceRef.current = null
    }
  }, [])

  const formatDuration = useCallback((startedAt: number) => {
    const now = Date.now()
    const seconds = Math.floor((now - startedAt) / 1000)
    if (seconds < 60) return `${seconds}s`
    const minutes = Math.floor(seconds / 60)
    return `${minutes}m${seconds % 60}s`
  }, [tick]) // tick 变化触发重算

  return (
    <Card
      title={
        <span>
          实时活跃密钥
          <Tag
            icon={connected ? <WifiOutlined /> : <DisconnectOutlined />}
            color={connected ? 'success' : 'error'}
            style={{ marginLeft: 8 }}
          >
            {connected ? 'SSE 已连接' : 'SSE 断开'}
          </Tag>
        </span>
      }
      className="section-card"
    >
      {entries.length === 0 ? (
        <Empty description="暂无活跃密钥" />
      ) : (
        <div className="active-keys-grid">
          {entries.map(entry => (
            <Badge.Ribbon
              key={entry.request_id}
              text={getPlatformLabel(entry.platform)}
              color={getPlatformColorHex(entry.platform)}
            >
              <Card size="small" className="active-key-card">
                <div className="active-key-info">
                  <div className="active-key-name">
                    {entry.key_name || entry.key_prefix || `Key #${entry.key_id}`}
                  </div>
                  <Tag>{entry.model}</Tag>
                  <div className="active-key-time">
                    已用时: {formatDuration(entry.started_at)}
                  </div>
                </div>
              </Card>
            </Badge.Ribbon>
          ))}
        </div>
      )}
    </Card>
  )
}
