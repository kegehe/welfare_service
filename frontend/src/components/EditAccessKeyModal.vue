<script setup lang="ts">
import { reactive, watch } from 'vue'
import type { AccessKey, UpdateAccessKeyInput } from '@/types'

const modelValue = defineModel<boolean>({ required: true })

const props = defineProps<{
  accessKey: AccessKey | null
}>()

const emit = defineEmits<{ submit: [id: number, input: UpdateAccessKeyInput] }>()

const form = reactive({
  name: '',
  rpm_limit: 0,
  tpm_limit: 0,
  expires_at: '',
})

function toDatePickerValue(value: string | null): string {
  return value ? value.replace(' ', 'T').substring(0, 19) : ''
}

function fillForm(key: AccessKey | null) {
  Object.assign(form, {
    name: key?.name || '',
    rpm_limit: key?.rpm_limit || 0,
    tpm_limit: key?.tpm_limit || 0,
    expires_at: toDatePickerValue(key?.expires_at || null),
  })
}

watch(
  () => [modelValue.value, props.accessKey] as const,
  ([visible, key]) => {
    if (visible) fillForm(key)
  },
  { immediate: true },
)

function handleSubmit() {
  if (!props.accessKey) return
  const input: UpdateAccessKeyInput = {
    name: form.name.trim() || null,
    rpm_limit: form.rpm_limit || 0,
    tpm_limit: form.tpm_limit || 0,
    expires_at: form.expires_at ? form.expires_at.replace('T', ' ') : null,
  }
  emit('submit', props.accessKey.id, input)
}
</script>

<template>
  <el-dialog v-model="modelValue" title="编辑访问 Key" width="520px">
    <el-form :model="form" label-width="120px" label-position="top">
      <el-row :gutter="16">
        <el-col :span="8">
          <el-form-item label="ID">
            <el-input :model-value="accessKey?.id ?? ''" disabled />
          </el-form-item>
        </el-col>
        <el-col :span="16">
          <el-form-item label="状态">
            <el-input :model-value="accessKey?.status || ''" disabled />
          </el-form-item>
        </el-col>
      </el-row>
      <el-form-item label="访问 Key">
        <el-input :model-value="accessKey?.key || ''" disabled />
      </el-form-item>
      <el-form-item label="名称">
        <el-input v-model="form.name" placeholder="如：张三的 Key" />
      </el-form-item>
      <el-row :gutter="16">
        <el-col :span="12">
          <el-form-item label="RPM 限制 (0=不限)">
            <el-input-number v-model="form.rpm_limit" :min="0" />
          </el-form-item>
        </el-col>
        <el-col :span="12">
          <el-form-item label="TPM 限制 (0=不限)">
            <el-input-number v-model="form.tpm_limit" :min="0" />
          </el-form-item>
        </el-col>
      </el-row>
      <el-form-item label="过期时间">
        <el-date-picker
          v-model="form.expires_at"
          type="datetime"
          placeholder="留空则永不过期"
          value-format="YYYY-MM-DDTHH:mm:ss"
        />
      </el-form-item>
      <el-row :gutter="16">
        <el-col :span="12">
          <el-form-item label="最后使用">
            <el-input :model-value="accessKey?.last_used_at || '-'" disabled />
          </el-form-item>
        </el-col>
        <el-col :span="12">
          <el-form-item label="创建时间">
            <el-input :model-value="accessKey?.created_at || '-'" disabled />
          </el-form-item>
        </el-col>
      </el-row>
    </el-form>
    <template #footer>
      <el-button @click="modelValue = false">取消</el-button>
      <el-button type="success" @click="handleSubmit">保存</el-button>
    </template>
  </el-dialog>
</template>
