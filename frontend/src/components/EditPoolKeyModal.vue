<script setup lang="ts">
import { reactive, watch } from 'vue'
import { ElMessage } from 'element-plus'
import type { PoolKey, UpdatePoolKeyInput } from '@/types'

const modelValue = defineModel<boolean>({ required: true })

const props = defineProps<{
  poolKey: PoolKey | null
}>()

const emit = defineEmits<{ submit: [id: number, input: UpdatePoolKeyInput] }>()

const form = reactive({
  platform: 'xiaomi',
  name: '',
  api_key: '',
  openai_url: '',
  claude_url: '',
  models: '',
  tpm_limit: 0,
  rpm_limit: 0,
  source: '',
  note: '',
})

function fillForm(key: PoolKey | null) {
  Object.assign(form, {
    platform: key?.platform || 'xiaomi',
    name: key?.name || '',
    api_key: key?.api_key || '',
    openai_url: key?.openai_url || '',
    claude_url: key?.claude_url || '',
    models: key?.models.join(', ') || '',
    tpm_limit: key?.tpm_limit || 0,
    rpm_limit: key?.rpm_limit || 0,
    source: key?.source || '',
    note: key?.note || '',
  })
}

watch(
  () => [modelValue.value, props.poolKey] as const,
  ([visible, key]) => {
    if (visible) fillForm(key)
  },
  { immediate: true },
)

function handleSubmit() {
  if (!props.poolKey) return
  if (!form.openai_url.trim() && !form.claude_url.trim()) {
    ElMessage.warning('OpenAI URL 和 Claude URL 至少填写一个')
    return
  }

  const modelList = form.models.split(',').map(s => s.trim()).filter(Boolean)
  if (modelList.length === 0) {
    ElMessage.warning('请输入至少一个模型')
    return
  }

  emit('submit', props.poolKey.id, {
    platform: form.platform,
    name: form.name.trim() || null,
    api_key: form.api_key.trim() || null,
    openai_url: form.openai_url.trim(),
    claude_url: form.claude_url.trim(),
    models: modelList,
    tpm_limit: form.tpm_limit || 0,
    rpm_limit: form.rpm_limit || 0,
    source: form.source.trim() || null,
    note: form.note.trim() || null,
  })
}
</script>

<template>
  <el-dialog v-model="modelValue" title="编辑号池 Key" width="640px">
    <el-form :model="form" label-width="100px" label-position="top">
      <el-row :gutter="16">
        <el-col :span="12">
          <el-form-item label="ID">
            <el-input :model-value="poolKey?.id ?? ''" disabled />
          </el-form-item>
        </el-col>
        <el-col :span="12">
          <el-form-item label="表格显示">
            <el-input :model-value="poolKey?.key_prefix || ''" disabled />
          </el-form-item>
        </el-col>
      </el-row>
      <el-row :gutter="16">
        <el-col :span="12">
          <el-form-item label="平台" required>
            <el-select v-model="form.platform">
              <el-option label="小米 (xiaomi)" value="xiaomi" />
              <el-option label="讯飞 (iflytek)" value="iflytek" />
            </el-select>
          </el-form-item>
        </el-col>
        <el-col :span="12">
          <el-form-item label="名称">
            <el-input v-model="form.name" placeholder="可选，用于识别该 Key" />
          </el-form-item>
        </el-col>
      </el-row>
      <el-form-item label="API Key">
        <el-input v-model="form.api_key" placeholder="留空则保留当前密钥" />
      </el-form-item>
      <el-row :gutter="16">
        <el-col :span="12">
          <el-form-item label="OpenAI URL">
            <el-input v-model="form.openai_url" placeholder="https://api.example.com/v1" />
          </el-form-item>
        </el-col>
        <el-col :span="12">
          <el-form-item label="Claude URL">
            <el-input v-model="form.claude_url" placeholder="https://api.example.com" />
          </el-form-item>
        </el-col>
      </el-row>
      <el-text type="info" size="small">
        OpenAI URL 和 Claude URL 至少填写一个；只配置一种时，该 Key 只用于对应协议。
      </el-text>
      <el-form-item label="上游可用模型 (逗号分隔)" required>
        <el-input v-model="form.models" placeholder="mimo-v2.5-pro, astron-code-latest" />
        <el-text type="info" size="small">
          客户端请求任意模型时，服务会自动映射到这里配置的可用模型。
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
      <el-row :gutter="16">
        <el-col :span="12">
          <el-form-item label="状态">
            <el-input :model-value="poolKey?.status || ''" disabled />
          </el-form-item>
        </el-col>
        <el-col :span="12">
          <el-form-item label="创建时间">
            <el-input :model-value="poolKey?.created_at || ''" disabled />
          </el-form-item>
        </el-col>
      </el-row>
    </el-form>
    <template #footer>
      <el-button @click="modelValue = false">取消</el-button>
      <el-button type="primary" @click="handleSubmit">保存</el-button>
    </template>
  </el-dialog>
</template>
