import { useState, useEffect, useCallback, useMemo } from 'react'
import { Layout, Tabs, message } from 'antd'
import { ReloadOutlined } from '@ant-design/icons'
import { useAppStore } from '@/stores/useAppStore'
import { useAutoRefresh } from '@/hooks/useAutoRefresh'
import { StatsOverview } from '@/components/overview/StatsOverview'
import { ActiveKeysBar } from '@/components/overview/ActiveKeysBar'
import { UsageStats } from '@/components/usage/UsageStats'
import { PoolKeysTable } from '@/components/pool/PoolKeysTable'
import { PoolStatsBar } from '@/components/pool/PoolStatsBar'
import { AccessKeysTable } from '@/components/access/AccessKeysTable'
import { ChatTest } from '@/components/chat/ChatTest'
import { AddPoolKeyModal } from '@/components/pool/AddPoolKeyModal'
import { EditPoolKeyModal } from '@/components/pool/EditPoolKeyModal'
import { CreateAccessKeyModal } from '@/components/access/CreateAccessKeyModal'
import { EditAccessKeyModal } from '@/components/access/EditAccessKeyModal'
import { ShowKeyModal } from '@/components/access/ShowKeyModal'
import { TestResultModal } from '@/components/common/TestResultModal'
import type { PoolKey, KeyStatus, KeyHealthScore, AccessKey, TestKeyResult, AddPoolKeyInput, UpdatePoolKeyInput, CreateAccessKeyInput, UpdateAccessKeyInput } from '@/types'
import { deletePoolKey, togglePoolKey, testPoolKey, addPoolKey, updatePoolKey, createAccessKey, updateAccessKey, deleteAccessKey, toggleAccessKey } from '@/api/admin'

const { Header, Content } = Layout

export default function App() {
  const {
    poolKeys, keyStatuses, healthScores, accessKeys, healthInfo, loading,
    healthScoreError, activeTab, setActiveTab, loadData
  } = useAppStore()

  // 弹窗状态
  const [showAddPoolKey, setShowAddPoolKey] = useState(false)
  const [showEditPoolKey, setShowEditPoolKey] = useState(false)
  const [showCreateAccessKey, setShowCreateAccessKey] = useState(false)
  const [showEditAccessKey, setShowEditAccessKey] = useState(false)
  const [showNewKey, setShowNewKey] = useState(false)
  const [showTestResult, setShowTestResult] = useState(false)
  const [editingPoolKey, setEditingPoolKey] = useState<PoolKey | null>(null)
  const [editingAccessKey, setEditingAccessKey] = useState<AccessKey | null>(null)
  const [newKeyValue, setNewKeyValue] = useState('')
  const [testResult, setTestResult] = useState<TestKeyResult | null>(null)
  const [testingKeyId, setTestingKeyId] = useState<number | null>(null)

  // 计算属性
  const statusMap = useMemo(() => {
    const map: Record<number, KeyStatus> = {}
    keyStatuses.forEach(s => { map[s.key_id] = s })
    return map
  }, [keyStatuses])

  const healthScoreMap = useMemo(() => {
    const map: Record<number, KeyHealthScore> = {}
    healthScores.forEach(s => { map[s.key_id] = s })
    return map
  }, [healthScores])

  const circuitOpenCount = useMemo(() =>
    keyStatuses.filter(s => s.circuit_state === 'open').length
  , [keyStatuses])

  const activePoolKeys = useMemo(() =>
    poolKeys.filter(k => k.status === 'active').length
  , [poolKeys])

  const version = useMemo(() =>
    healthInfo ? `v${healthInfo.version} · Welfare Service` : 'API Key 池化共享服务'
  , [healthInfo])

  const baseUrl = useMemo(() => healthInfo?.base_url || '', [healthInfo])

  const availableModels = useMemo(() => {
    const models = new Set<string>()
    poolKeys.forEach(key => {
      if (key.status !== 'active') return
      key.models.forEach(model => {
        const trimmed = model.trim()
        if (trimmed) models.add(trimmed)
      })
    })
    return Array.from(models).sort()
  }, [poolKeys])

  // 自动刷新（每10秒）
  useAutoRefresh(loadData, 10000)

  // 初始加载
  useEffect(() => {
    loadData()
  }, [loadData])

  // 号池 Key 操作
  const handleDeletePoolKey = useCallback(async (id: number) => {
    try {
      await deletePoolKey(id)
      message.success('删除成功')
      loadData()
    } catch (e: any) {
      message.error('删除失败: ' + (e.message || '未知错误'))
    }
  }, [loadData])

  const handleTogglePoolKey = useCallback(async (id: number) => {
    try {
      await togglePoolKey(id)
      message.success('状态已切换')
      loadData()
    } catch (e: any) {
      message.error('操作失败: ' + (e.message || '未知错误'))
    }
  }, [loadData])

  const handleTestPoolKey = useCallback(async (id: number) => {
    setTestingKeyId(id)
    try {
      const result = await testPoolKey(id)
      setTestResult(result)
      setShowTestResult(true)
    } catch (e: any) {
      message.error('测试失败: ' + (e.message || '未知错误'))
    } finally {
      setTestingKeyId(null)
    }
  }, [])

  const handleAddPoolKey = useCallback(async (input: AddPoolKeyInput) => {
    try {
      await addPoolKey(input)
      message.success('添加成功')
      setShowAddPoolKey(false)
      loadData()
    } catch (e: any) {
      message.error('添加失败: ' + (e.message || '未知错误'))
    }
  }, [loadData])

  const handleEditPoolKey = useCallback((key: PoolKey) => {
    setEditingPoolKey(key)
    setShowEditPoolKey(true)
  }, [])

  const handleUpdatePoolKey = useCallback(async (id: number, input: UpdatePoolKeyInput) => {
    try {
      await updatePoolKey(id, input)
      message.success('保存成功')
      setShowEditPoolKey(false)
      setEditingPoolKey(null)
      loadData()
    } catch (e: any) {
      message.error('保存失败: ' + (e.message || '未知错误'))
    }
  }, [loadData])

  // 访问 Key 操作
  const handleDeleteAccessKey = useCallback(async (id: number) => {
    try {
      await deleteAccessKey(id)
      message.success('删除成功')
      loadData()
    } catch (e: any) {
      message.error('删除失败: ' + (e.message || '未知错误'))
    }
  }, [loadData])

  const handleToggleAccessKey = useCallback(async (id: number) => {
    try {
      await toggleAccessKey(id)
      message.success('状态已切换')
      loadData()
    } catch (e: any) {
      message.error('操作失败: ' + (e.message || '未知错误'))
    }
  }, [loadData])

  const handleCreateAccessKey = useCallback(async (input: CreateAccessKeyInput) => {
    try {
      const result = await createAccessKey(input)
      setShowCreateAccessKey(false)
      setNewKeyValue(result.key)
      setShowNewKey(true)
      loadData()
    } catch (e: any) {
      message.error('创建失败: ' + (e.message || '未知错误'))
    }
  }, [loadData])

  const handleEditAccessKey = useCallback((key: AccessKey) => {
    setEditingAccessKey(key)
    setShowEditAccessKey(true)
  }, [])

  const handleUpdateAccessKey = useCallback(async (id: number, input: UpdateAccessKeyInput) => {
    try {
      await updateAccessKey(id, input)
      message.success('保存成功')
      setShowEditAccessKey(false)
      setEditingAccessKey(null)
      loadData()
    } catch (e: any) {
      message.error('保存失败: ' + (e.message || '未知错误'))
    }
  }, [loadData])

  // 号池容量百分比 + 健康状态分级
  const poolCapacityPercent = useMemo(() =>
    poolKeys.length > 0 ? (activePoolKeys / poolKeys.length) * 100 : 0
  , [activePoolKeys, poolKeys.length])

  const unhealthyCount = useMemo(() =>
    keyStatuses.filter(s => s.circuit_state !== 'closed').length
  , [keyStatuses])

  const degradedHealthCount = useMemo(() =>
    healthScores.filter(s =>
      s.status_label === 'light_throttled' ||
      s.status_label === 'heavy_throttled' ||
      s.status_label === 'critical'
    ).length
  , [healthScores])

  const criticalHealthCount = useMemo(() =>
    healthScores.filter(s =>
      s.status_label === 'heavy_throttled' ||
      s.status_label === 'critical'
    ).length
  , [healthScores])

  const headerState = useMemo(() => {
    if (circuitOpenCount > 0 || criticalHealthCount > 0) return 'critical'
    if (unhealthyCount > 0 || degradedHealthCount > 0 || healthScoreError) return 'warning'
    return 'healthy'
  }, [circuitOpenCount, criticalHealthCount, unhealthyCount, degradedHealthCount, healthScoreError])

  const tabItems = [
    {
      key: 'overview',
      label: '总览',
      children: (
        <>
          <StatsOverview
            poolKeys={poolKeys}
            keyStatuses={keyStatuses}
            healthScores={healthScores}
            healthScoreError={healthScoreError}
            accessKeys={accessKeys}
            version={version}
          />
          <ActiveKeysBar />
        </>
      ),
    },
    {
      key: 'usage',
      label: '用量统计',
      children: <UsageStats />,
    },
    {
      key: 'pool',
      label: '号池管理',
      children: (
        <>
          <PoolStatsBar
            poolKeys={poolKeys}
            keyStatuses={keyStatuses}
            healthScores={healthScores}
          />
          <PoolKeysTable
            poolKeys={poolKeys}
            statusMap={statusMap}
            healthScoreMap={healthScoreMap}
            testingKeyId={testingKeyId}
            onTest={handleTestPoolKey}
            onAdd={() => setShowAddPoolKey(true)}
            onEdit={handleEditPoolKey}
            onDelete={handleDeletePoolKey}
            onToggle={handleTogglePoolKey}
          />
        </>
      ),
    },
    {
      key: 'access',
      label: '访问 Key',
      children: (
        <AccessKeysTable
          accessKeys={accessKeys}
          baseUrl={baseUrl}
          availableModels={availableModels}
          onDelete={handleDeleteAccessKey}
          onToggle={handleToggleAccessKey}
          onEdit={handleEditAccessKey}
          onCreate={() => setShowCreateAccessKey(true)}
        />
      ),
    },
    {
      key: 'chat',
      label: '对话测试',
      children: <ChatTest accessKeys={accessKeys} />,
    },
  ]

  return (
    <Layout className="app-layout">
      <Header className={`app-header pool-${headerState}`}>
        <div className="app-header-content">
          <div>
            <h1>Welfare Service</h1>
            <div className="header-subtitle">
              <div className="pool-capacity-bar">
                <div
                  className={`pool-capacity-bar-fill${headerState !== 'healthy' ? ` ${headerState}` : ''}`}
                  style={{ width: `${poolCapacityPercent}%` }}
                />
              </div>
              <span className="version">
                {poolKeys.length > 0
                  ? `${activePoolKeys}/${poolKeys.length} 可用`
                  : version}
              </span>
              {poolKeys.length > 0 && healthInfo && (
                <span className="version">{version}</span>
              )}
            </div>
          </div>
          <button
            className="header-refresh-btn"
            onClick={() => loadData()}
            disabled={loading}
          >
            <ReloadOutlined spin={loading} /> 刷新
          </button>
        </div>
      </Header>

      <Content className="app-container">
        <Tabs
          activeKey={activeTab}
          onChange={setActiveTab}
          items={tabItems}
          className="main-tabs"
        />
      </Content>

      {/* 弹窗 */}
      <AddPoolKeyModal
        open={showAddPoolKey}
        onClose={() => setShowAddPoolKey(false)}
        onSubmit={handleAddPoolKey}
      />
      <EditPoolKeyModal
        open={showEditPoolKey}
        poolKey={editingPoolKey}
        onClose={() => {
          setShowEditPoolKey(false)
          setEditingPoolKey(null)
        }}
        onSubmit={handleUpdatePoolKey}
      />
      <CreateAccessKeyModal
        open={showCreateAccessKey}
        onClose={() => setShowCreateAccessKey(false)}
        onSubmit={handleCreateAccessKey}
      />
      <EditAccessKeyModal
        open={showEditAccessKey}
        accessKey={editingAccessKey}
        onClose={() => {
          setShowEditAccessKey(false)
          setEditingAccessKey(null)
        }}
        onSubmit={handleUpdateAccessKey}
      />
      <ShowKeyModal
        open={showNewKey}
        newKey={newKeyValue}
        baseUrl={baseUrl}
        availableModels={availableModels}
        onClose={() => setShowNewKey(false)}
      />
      <TestResultModal
        open={showTestResult}
        result={testResult}
        onClose={() => setShowTestResult(false)}
      />
    </Layout>
  )
}
