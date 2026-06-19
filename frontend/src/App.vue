<script setup lang="ts">
import { ref, computed, provide } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useAutoRefresh } from '@/composables/useAutoRefresh'
import {
  listPoolKeys, getKeyStatuses, listAccessKeys, getHealth,
  deletePoolKey, togglePoolKey, testPoolKey, addPoolKey, updatePoolKey,
  createAccessKey, updateAccessKey, deleteAccessKey, toggleAccessKey,
} from '@/api/admin'
import type {
  PoolKey, KeyStatus, AccessKey, HealthInfo, TestKeyResult,
  AddPoolKeyInput, UpdatePoolKeyInput, CreateAccessKeyInput, UpdateAccessKeyInput,
} from '@/types'
import StatsOverview from '@/components/StatsOverview.vue'
import PoolKeysTable from '@/components/PoolKeysTable.vue'
import AccessKeysTable from '@/components/AccessKeysTable.vue'
import ChatTest from '@/components/ChatTest.vue'
import AddPoolKeyModal from '@/components/AddPoolKeyModal.vue'
import EditPoolKeyModal from '@/components/EditPoolKeyModal.vue'
import CreateAccessKeyModal from '@/components/CreateAccessKeyModal.vue'
import EditAccessKeyModal from '@/components/EditAccessKeyModal.vue'
import ShowKeyModal from '@/components/ShowKeyModal.vue'
import TestResultModal from '@/components/TestResultModal.vue'

// 全局数据
const poolKeys = ref<PoolKey[]>([])
const keyStatuses = ref<KeyStatus[]>([])
const accessKeys = ref<AccessKey[]>([])
const healthInfo = ref<HealthInfo | null>(null)
const loading = ref(false)

// 弹窗状态
const showAddPoolKey = ref(false)
const showEditPoolKey = ref(false)
const showCreateAccessKey = ref(false)
const showEditAccessKey = ref(false)
const showNewKey = ref(false)
const showTestResult = ref(false)
const editingPoolKey = ref<PoolKey | null>(null)
const editingAccessKey = ref<AccessKey | null>(null)
const newKeyValue = ref('')
const testResult = ref<TestKeyResult | null>(null)
const testingKeyId = ref<number | null>(null)

// 计算属性
const statusMap = computed(() => {
  const map: Record<number, KeyStatus> = {}
  keyStatuses.value.forEach(s => { map[s.key_id] = s })
  return map
})

const circuitOpenCount = computed(() =>
  keyStatuses.value.filter(s => s.circuit_state === 'open').length
)

const version = computed(() => healthInfo.value ? `v${healthInfo.value.version} · Welfare Service` : 'API Key 池化共享服务')

const baseUrl = computed(() => healthInfo.value?.base_url || '')

const availableModels = computed(() => {
  const models = new Set<string>()
  poolKeys.value.forEach(key => {
    if (key.status !== 'active') return
    key.models.forEach(model => {
      const trimmed = model.trim()
      if (trimmed) models.add(trimmed)
    })
  })
  return Array.from(models).sort()
})

// 数据加载
async function loadData() {
  loading.value = true
  try {
    const [poolData, healthData, poolStatus, accessData] = await Promise.all([
      listPoolKeys(),
      getHealth(),
      getKeyStatuses(),
      listAccessKeys(),
    ])
    poolKeys.value = poolData.keys || []
    keyStatuses.value = poolStatus.statuses || []
    accessKeys.value = accessData.keys || []
    healthInfo.value = healthData
  } catch (e: any) {
    ElMessage.error('加载失败: ' + (e.message || '未知错误'))
  } finally {
    loading.value = false
  }
}

// 自动刷新
useAutoRefresh(loadData, 10000)

// provide 给子组件
provide('reloadData', loadData)

// 号池 Key 操作
async function handleDeletePoolKey(id: number) {
  try {
    await ElMessageBox.confirm(`确认删除号池 Key #${id}？`, '确认删除', { type: 'warning' })
    await deletePoolKey(id)
    ElMessage.success('删除成功')
    loadData()
  } catch (e: any) {
    if (e !== 'cancel') ElMessage.error('删除失败: ' + (e.message || '未知错误'))
  }
}

async function handleTogglePoolKey(id: number) {
  try {
    await togglePoolKey(id)
    ElMessage.success('状态已切换')
    loadData()
  } catch (e: any) {
    ElMessage.error('操作失败: ' + (e.message || '未知错误'))
  }
}

async function handleTestPoolKey(id: number) {
  testingKeyId.value = id
  try {
    const result = await testPoolKey(id)
    testResult.value = result
    showTestResult.value = true
  } catch (e: any) {
    ElMessage.error('测试失败: ' + (e.message || '未知错误'))
  } finally {
    testingKeyId.value = null
  }
}

async function handleAddPoolKey(input: AddPoolKeyInput) {
  try {
    await addPoolKey(input)
    ElMessage.success('添加成功')
    showAddPoolKey.value = false
    loadData()
  } catch (e: any) {
    ElMessage.error('添加失败: ' + (e.message || '未知错误'))
  }
}

function handleEditPoolKey(key: PoolKey) {
  editingPoolKey.value = key
  showEditPoolKey.value = true
}

async function handleUpdatePoolKey(id: number, input: UpdatePoolKeyInput) {
  try {
    await updatePoolKey(id, input)
    ElMessage.success('保存成功')
    showEditPoolKey.value = false
    editingPoolKey.value = null
    loadData()
  } catch (e: any) {
    ElMessage.error('保存失败: ' + (e.message || '未知错误'))
  }
}

// 访问 Key 操作
async function handleDeleteAccessKey(id: number) {
  try {
    await ElMessageBox.confirm(`确认删除访问 Key #${id}？`, '确认删除', { type: 'warning' })
    await deleteAccessKey(id)
    ElMessage.success('删除成功')
    loadData()
  } catch (e: any) {
    if (e !== 'cancel') ElMessage.error('删除失败: ' + (e.message || '未知错误'))
  }
}

async function handleToggleAccessKey(id: number) {
  try {
    await toggleAccessKey(id)
    ElMessage.success('状态已切换')
    loadData()
  } catch (e: any) {
    ElMessage.error('操作失败: ' + (e.message || '未知错误'))
  }
}

async function handleCreateAccessKey(input: CreateAccessKeyInput) {
  try {
    const result = await createAccessKey(input)
    showCreateAccessKey.value = false
    newKeyValue.value = result.key
    showNewKey.value = true
    loadData()
  } catch (e: any) {
    ElMessage.error('创建失败: ' + (e.message || '未知错误'))
  }
}

function handleEditAccessKey(key: AccessKey) {
  editingAccessKey.value = key
  showEditAccessKey.value = true
}

async function handleUpdateAccessKey(id: number, input: UpdateAccessKeyInput) {
  try {
    await updateAccessKey(id, input)
    ElMessage.success('保存成功')
    showEditAccessKey.value = false
    editingAccessKey.value = null
    loadData()
  } catch (e: any) {
    ElMessage.error('保存失败: ' + (e.message || '未知错误'))
  }
}

// 初始加载
loadData()
</script>

<template>
  <el-container class="app-layout">
    <!-- 顶部 Header -->
    <el-header class="app-header" :class="{ 'pool-alert': circuitOpenCount > 0 }">
      <el-row justify="space-between" align="middle" class="app-header-row">
        <el-col>
          <h1>🔑 Welfare Service</h1>
          <el-text class="version" size="small">{{ version }}</el-text>
        </el-col>
        <el-col>
          <el-button :loading="loading" plain class="header-refresh-btn" @click="loadData()">🔄 刷新</el-button>
        </el-col>
      </el-row>
    </el-header>

    <el-main class="app-container">
      <!-- 统计卡片 -->
      <StatsOverview
        :pool-keys="poolKeys"
        :key-statuses="keyStatuses"
        :access-keys="accessKeys"
        :version="version"
      />

      <!-- 号池 Key -->
      <PoolKeysTable
        :pool-keys="poolKeys"
        :status-map="statusMap"
        :testing-key-id="testingKeyId"
        @delete="handleDeletePoolKey"
        @toggle="handleTogglePoolKey"
        @test="handleTestPoolKey"
        @edit="handleEditPoolKey"
        @add="showAddPoolKey = true"
      />

      <!-- 访问 Key -->
      <AccessKeysTable
        :access-keys="accessKeys"
        :base-url="baseUrl"
        :available-models="availableModels"
        @delete="handleDeleteAccessKey"
        @toggle="handleToggleAccessKey"
        @edit="handleEditAccessKey"
        @create="showCreateAccessKey = true"
      />

      <!-- 对话测试 -->
      <ChatTest :access-keys="accessKeys" />
    </el-main>

    <!-- 弹窗 -->
    <AddPoolKeyModal v-model="showAddPoolKey" @submit="handleAddPoolKey" />
    <EditPoolKeyModal
      v-model="showEditPoolKey"
      :pool-key="editingPoolKey"
      @submit="handleUpdatePoolKey"
    />
    <CreateAccessKeyModal v-model="showCreateAccessKey" @submit="handleCreateAccessKey" />
    <EditAccessKeyModal
      v-model="showEditAccessKey"
      :access-key="editingAccessKey"
      @submit="handleUpdateAccessKey"
    />
    <ShowKeyModal
      v-model="showNewKey"
      :new-key="newKeyValue"
      :base-url="baseUrl"
      :available-models="availableModels"
    />
    <TestResultModal v-model="showTestResult" :result="testResult" />
  </el-container>
</template>

<style scoped>
.app-layout {
  min-height: 100vh;
}

.app-header {
  background: var(--ws-conduit);
  color: var(--header-text);
  height: auto;
  padding: 16px 32px;
  position: relative;
  overflow: hidden;
  border-bottom: none;
}

/* Left accent edge */
.app-header::before {
  content: '';
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 3px;
  background: var(--ws-pool);
  transition: background 0.5s ease;
}

/* Pool pulse bar — always present at bottom */
.app-header::after {
  content: '';
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  height: 3px;
  background: var(--ws-pool);
  transition: background 0.5s ease, box-shadow 0.5s ease;
}

/* Alert state — circuit(s) open: bar turns amber and pulses */
.pool-alert::after {
  background: var(--ws-fuse);
  animation: pool-pulse 2s ease-in-out infinite;
}

.pool-alert::before {
  background: var(--ws-fuse);
}

@keyframes pool-pulse {
  0%, 100% {
    opacity: 1;
    box-shadow: 0 0 12px rgba(var(--ws-fuse-rgb), 0.4);
  }
  50% {
    opacity: 0.6;
    box-shadow: 0 0 4px rgba(var(--ws-fuse-rgb), 0.2);
  }
}

.app-header-row {
  width: 100%;
}

.app-header h1 {
  font-size: var(--text-xl);
  font-weight: var(--weight-bold);
  margin: 0;
  color: var(--header-text);
  letter-spacing: -0.01em;
}

.app-header :deep(.el-text) {
  color: rgba(var(--ws-channel-rgb), 0.5);
  font-size: var(--text-sm);
  font-family: var(--font-mono);
  letter-spacing: 0.02em;
}

.header-refresh-btn {
  color: var(--ws-pool-light) !important;
  border-color: rgba(var(--ws-pool-rgb), 0.4) !important;
  background: transparent !important;
}

.header-refresh-btn:hover {
  color: var(--header-text) !important;
  border-color: var(--ws-pool-light) !important;
  background-color: rgba(var(--ws-pool-rgb), 0.15) !important;
}

.app-container {
  max-width: 1200px;
  margin: 0 auto;
  padding: 24px;
}
</style>
