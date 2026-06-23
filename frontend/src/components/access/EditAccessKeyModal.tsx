import { useState, useEffect } from 'react'
import { Modal, Form, Input, InputNumber, DatePicker, Descriptions, Tag, message } from 'antd'
import dayjs from 'dayjs'
import type { AccessKey, UpdateAccessKeyInput } from '@/types'

interface Props {
  open: boolean
  accessKey: AccessKey | null
  onClose: () => void
  onSubmit: (id: number, input: UpdateAccessKeyInput) => Promise<void>
}

export function EditAccessKeyModal({ open, accessKey, onClose, onSubmit }: Props) {
  const [form] = Form.useForm()
  const [loading, setLoading] = useState(false)

  useEffect(() => {
    if (accessKey && open) {
      form.setFieldsValue({
        name: accessKey.name,
        rpm_limit: accessKey.rpm_limit,
        tpm_limit: accessKey.tpm_limit,
        expires_at: accessKey.expires_at ? dayjs(accessKey.expires_at) : null,
      })
    }
  }, [accessKey, open, form])

  const handleSubmit = async () => {
    if (!accessKey) return
    try {
      const values = await form.validateFields()
      setLoading(true)
      // 日期格式：YYYY-MM-DD HH:mm:ss（与 Vue 版本一致）
      const expiresAt = values.expires_at
        ? values.expires_at.format('YYYY-MM-DD HH:mm:ss')
        : null
      await onSubmit(accessKey.id, {
        name: values.name?.trim() || null,
        rpm_limit: values.rpm_limit || 0,
        tpm_limit: values.tpm_limit || 0,
        expires_at: expiresAt,
      })
    } catch (e: any) {
      if (e.errorFields) return
      message.error('提交失败: ' + (e.message || '未知错误'))
    } finally {
      setLoading(false)
    }
  }

  const formatTime = (t: string | null) => {
    if (!t) return '-'
    return new Date(t).toLocaleString('zh-CN')
  }

  return (
    <Modal
      title={`编辑访问 Key #${accessKey?.id}`}
      open={open}
      onCancel={() => {
        form.resetFields()
        onClose()
      }}
      onOk={handleSubmit}
      confirmLoading={loading}
      width={500}
    >
      {/* 只读信息 */}
      {accessKey && (
        <Descriptions column={2} size="small" bordered className="config-section">
          <Descriptions.Item label="ID">{accessKey.id}</Descriptions.Item>
          <Descriptions.Item label="Key">
            <code className="text-label">{accessKey.key.slice(0, 12)}...</code>
          </Descriptions.Item>
          <Descriptions.Item label="状态">
            <Tag color={accessKey.status === 'active' ? 'success' : 'error'}>{accessKey.status}</Tag>
          </Descriptions.Item>
          <Descriptions.Item label="最后使用">{formatTime(accessKey.last_used_at)}</Descriptions.Item>
          <Descriptions.Item label="创建时间" span={2}>{formatTime(accessKey.created_at)}</Descriptions.Item>
        </Descriptions>
      )}

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
