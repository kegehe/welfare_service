import { useState } from 'react'
import { Modal, Form, Input, InputNumber, DatePicker, message } from 'antd'
import type { CreateAccessKeyInput } from '@/types'

interface Props {
  open: boolean
  onClose: () => void
  onSubmit: (input: CreateAccessKeyInput) => Promise<void>
}

export function CreateAccessKeyModal({ open, onClose, onSubmit }: Props) {
  const [form] = Form.useForm()
  const [loading, setLoading] = useState(false)

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields()
      setLoading(true)
      // 日期格式：YYYY-MM-DD HH:mm:ss（与 Vue 版本一致）
      const expiresAt = values.expires_at
        ? values.expires_at.format('YYYY-MM-DD HH:mm:ss')
        : null
      await onSubmit({
        name: values.name?.trim() || null,
        rpm_limit: values.rpm_limit || 0,
        tpm_limit: values.tpm_limit || 0,
        expires_at: expiresAt,
      })
      form.resetFields()
    } catch (e: any) {
      if (e.errorFields) return
      message.error('提交失败: ' + (e.message || '未知错误'))
    } finally {
      setLoading(false)
    }
  }

  return (
    <Modal
      title="创建访问 Key"
      open={open}
      onCancel={() => {
        form.resetFields()
        onClose()
      }}
      onOk={handleSubmit}
      confirmLoading={loading}
      width={500}
    >
      <Form form={form} layout="vertical">
        <Form.Item name="name" label="名称">
          <Input placeholder="给这个 Key 起个名字（可选）" />
        </Form.Item>

        <div className="form-row">
          <Form.Item name="rpm_limit" label="RPM 限制">
            <InputNumber min={0} placeholder="0 = 不限" />
          </Form.Item>
          <Form.Item name="tpm_limit" label="TPM 限制">
            <InputNumber min={0} placeholder="0 = 不限" />
          </Form.Item>
        </div>

        <Form.Item name="expires_at" label="过期时间">
          <DatePicker showTime placeholder="选择过期时间（可选）" />
        </Form.Item>
      </Form>
    </Modal>
  )
}
