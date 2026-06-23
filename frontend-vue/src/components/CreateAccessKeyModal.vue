<script setup lang="ts">
import { reactive } from 'vue'
import type { CreateAccessKeyInput } from '@/types'

const modelValue = defineModel<boolean>({ required: true })
const emit = defineEmits<{ submit: [input: CreateAccessKeyInput] }>()

const form = reactive({
  name: '',
  rpm_limit: 0,
  tpm_limit: 0,
  expires_at: '',
})

function handleSubmit() {
  const expireVal = form.expires_at
  const input: CreateAccessKeyInput = {
    name: form.name || null,
    rpm_limit: form.rpm_limit || 0,
    tpm_limit: form.tpm_limit || 0,
    expires_at: expireVal ? expireVal.replace('T', ' ') : null,
  }
  emit('submit', input)
}

function handleClosed() {
  Object.assign(form, { name: '', rpm_limit: 0, tpm_limit: 0, expires_at: '' })
}
</script>

<template>
  <el-dialog v-model="modelValue" title="创建访问 Key" width="480px" @closed="handleClosed">
    <el-form :model="form" label-width="120px" label-position="top">
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
    </el-form>
    <template #footer>
      <el-button @click="modelValue = false">取消</el-button>
      <el-button type="success" @click="handleSubmit">创建</el-button>
    </template>
  </el-dialog>
</template>
