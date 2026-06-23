<script setup lang="ts">
import { reactive, computed, watch } from 'vue'
import { ElMessage } from 'element-plus'
import type { AddPoolKeyInput } from '@/types'
import { useModelPresets } from '@/composables/useModelPresets'

const modelValue = defineModel<boolean>({ required: true })
const emit = defineEmits<{ submit: [input: AddPoolKeyInput] }>()

const { ensurePresetsLoaded, getPresetModelsForPlatform } = useModelPresets()

const form = reactive({
  platform: 'xiaomi',
  name: '',
  api_key: '',
  openai_url: '',
  claude_url: '',
  models: [] as string[],
  tpm_limit: 0,
  rpm_limit: 0,
  source: '',
  note: '',
})

const openaiPlaceholder = computed(() => {
  if (form.platform === 'anthropic') return 'Anthropic 中转站通常不使用 OpenAI 协议'
  return 'https://api.example.com/v1'
})

const claudePlaceholder = computed(() => {
  if (form.platform === 'anthropic') return 'https://your-relay.example.com'
  return 'https://api.example.com'
})

const presetModels = computed(() => getPresetModelsForPlatform(form.platform))

// 对话框打开时加载预设数据
watch(modelValue, (visible) => {
  if (visible) ensurePresetsLoaded()
})

function handleSubmit() {
  // 基本验证
  if (!form.api_key.trim()) {
    ElMessage.warning('请输入 API Key')
    return
  }
  if (!form.openai_url.trim() && !form.claude_url.trim()) {
    ElMessage.warning('OpenAI URL 和 Claude URL 至少填写一个')
    return
  }
  // 清理 models：去除每项空白和空项
  const modelList = form.models.map(s => s.trim()).filter(Boolean)
  if (modelList.length === 0) {
    ElMessage.warning('请输入至少一个模型')
    return
  }
  const input: AddPoolKeyInput = {
    platform: form.platform,
    name: form.name.trim() || null,
    api_key: form.api_key.trim(),
    openai_url: form.openai_url.trim(),
    claude_url: form.claude_url.trim(),
    models: modelList,
    tpm_limit: form.tpm_limit || 0,
    rpm_limit: form.rpm_limit || 0,
    source: form.source.trim() || null,
    note: form.note.trim() || null,
  }
  emit('submit', input)
}

function handleClosed() {
  Object.assign(form, {
    platform: 'xiaomi',
    name: '',
    api_key: '',
    openai_url: '',
    claude_url: '',
    models: [],
    tpm_limit: 0,
    rpm_limit: 0,
    source: '',
    note: '',
  })
}
</script>

<template>
  <el-dialog v-model="modelValue" title="添加号池 Key" width="640px" @closed="handleClosed">
    <el-form :model="form" label-width="100px" label-position="top">
      <el-row :gutter="16">
        <el-col :span="12">
          <el-form-item label="平台" required>
            <el-select v-model="form.platform">
              <el-option label="小米 (xiaomi)" value="xiaomi" />
              <el-option label="讯飞 (iflytek)" value="iflytek" />
              <el-option label="Anthropic (anthropic)" value="anthropic" />
            </el-select>
          </el-form-item>
        </el-col>
        <el-col :span="12">
          <el-form-item label="名称">
            <el-input v-model="form.name" placeholder="可选，用于识别该 Key" />
          </el-form-item>
        </el-col>
      </el-row>
      <el-form-item label="API Key" required>
        <el-input v-model="form.api_key" placeholder="输入上游 API Key" />
      </el-form-item>
      <el-row :gutter="16">
        <el-col :span="12">
          <el-form-item label="OpenAI URL">
            <el-input v-model="form.openai_url" :placeholder="openaiPlaceholder" />
          </el-form-item>
        </el-col>
        <el-col :span="12">
          <el-form-item label="Claude URL">
            <el-input v-model="form.claude_url" :placeholder="claudePlaceholder" />
          </el-form-item>
        </el-col>
      </el-row>
      <el-text type="info" size="small">
        OpenAI URL 和 Claude URL 至少填写一个；只配置一种时，该 Key 只用于对应协议。
      </el-text>
      <el-form-item label="上游可用模型" required>
        <el-select
          v-model="form.models"
          multiple
          filterable
          allow-create
          default-first-option
          placeholder="选择或输入模型名称"
        >
          <el-option
            v-for="model in presetModels"
            :key="model"
            :label="model"
            :value="model"
          />
        </el-select>
        <el-text type="info" size="small">
          从下拉列表选择预设模型，或直接输入自定义模型名。
        </el-text>
      </el-form-item>
      <el-row :gutter="16">
        <el-col :span="12">
          <el-form-item label="TPM (0=不限)">
            <el-input-number v-model="form.tpm_limit" :min="0" />
          </el-form-item>
        </el-col>
        <el-col :span="12">
          <el-form-item label="RPM (0=不限)">
            <el-input-number v-model="form.rpm_limit" :min="0" />
          </el-form-item>
        </el-col>
      </el-row>
      <el-row :gutter="16">
        <el-col :span="12">
          <el-form-item label="来源">
            <el-input v-model="form.source" placeholder="可选" />
          </el-form-item>
        </el-col>
        <el-col :span="12">
          <el-form-item label="备注">
            <el-input v-model="form.note" placeholder="可选" />
          </el-form-item>
        </el-col>
      </el-row>
    </el-form>
    <template #footer>
      <el-button @click="modelValue = false">取消</el-button>
      <el-button type="primary" @click="handleSubmit">添加</el-button>
    </template>
  </el-dialog>
</template>
