import { useState, useEffect, useMemo } from 'react'
import { Modal, Form, Input, Select, InputNumber, Descriptions, Tag, message } from 'antd'
import type { PoolKey, UpdatePoolKeyInput } from '@/types'
import { useModelPresets } from '@/hooks/useModelPresets'

interface Props {
  open: boolean
  poolKey: PoolKey | null
  onClose: () => void
  onSubmit: (id: number, input: UpdatePoolKeyInput) => Promise<void>
}

export function EditPoolKeyModal({ open, poolKey, onClose, onSubmit }: Props) {
  const [form] = Form.useForm()
  const [loading, setLoading] = useState(false)
  const { ensurePresetsLoaded, getPresetModelsForPlatform } = useModelPresets()

  const platform = Form.useWatch('platform', form) || poolKey?.platform || 'xiaomi'

  useEffect(() => {
    if (open) ensurePresetsLoaded()
  }, [open, ensurePresetsLoaded])

  useEffect(() => {
    if (poolKey && open) {
      form.setFieldsValue({
        platform: poolKey.platform,
        name: poolKey.name,
        api_key: '', // 留空保留原密钥
        openai_url: poolKey.openai_url,
        claude_url: poolKey.claude_url,
        models: poolKey.models,
        tpm_limit: poolKey.tpm_limit,
        rpm_limit: poolKey.rpm_limit,
        source: poolKey.source,
        note: poolKey.note,
      })
    }
  }, [poolKey, open, form])

  const openaiPlaceholder = useMemo(() => {
    if (platform === 'anthropic') return 'Anthropic 中转站通常不使用 OpenAI 协议'
    return 'https://api.example.com/v1'
  }, [platform])

  const claudePlaceholder = useMemo(() => {
    if (platform === 'anthropic') return 'https://your-relay.example.com'
    return 'https://api.example.com'
  }, [platform])

  const presetModels = useMemo(() => getPresetModelsForPlatform(platform), [platform, getPresetModelsForPlatform])

  const handleSubmit = async () => {
    if (!poolKey) return
    try {
      const values = await form.validateFields()
      if (!values.openai_url?.trim() && !values.claude_url?.trim()) {
        message.warning('OpenAI URL 和 Claude URL 至少填写一个')
        return
      }
      const modelList = (values.models || []).map((s: string) => s.trim()).filter(Boolean)
      if (modelList.length === 0) {
        message.warning('请输入至少一个模型')
        return
      }

      setLoading(true)
      await onSubmit(poolKey.id, {
        platform: values.platform,
        name: values.name?.trim() || null,
        api_key: values.api_key?.trim() || null, // 留空则保留原密钥
        openai_url: values.openai_url || '',
        claude_url: values.claude_url || '',
        models: modelList,
        tpm_limit: values.tpm_limit || 0,
        rpm_limit: values.rpm_limit || 0,
        source: values.source?.trim() || null,
        note: values.note?.trim() || null,
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
      title={`编辑号池 Key #${poolKey?.id}`}
      open={open}
      onCancel={() => {
        form.resetFields()
        onClose()
      }}
      onOk={handleSubmit}
      confirmLoading={loading}
      width={600}
    >
      {/* 只读信息 */}
      {poolKey && (
        <Descriptions column={2} size="small" bordered className="config-section">
          <Descriptions.Item label="ID">{poolKey.id}</Descriptions.Item>
          <Descriptions.Item label="Key 前缀">
            <code>{poolKey.key_prefix}</code>
          </Descriptions.Item>
          <Descriptions.Item label="状态">
            <Tag color={poolKey.status === 'active' ? 'success' : 'error'}>{poolKey.status}</Tag>
          </Descriptions.Item>
          <Descriptions.Item label="创建时间">{formatTime(poolKey.created_at)}</Descriptions.Item>
        </Descriptions>
      )}

      <Form form={form} layout="vertical">
        <Form.Item name="platform" label="平台" rules={[{ required: true, message: '请选择平台' }]}>
          <Select placeholder="选择平台">
            <Select.Option value="xiaomi">小米</Select.Option>
            <Select.Option value="iflytek">讯飞</Select.Option>
            <Select.Option value="anthropic">Anthropic</Select.Option>
          </Select>
        </Form.Item>

        <Form.Item name="name" label="名称">
          <Input placeholder="给这个 Key 起个名字（可选）" />
        </Form.Item>

        <Form.Item name="api_key" label="API Key">
          <Input.Password placeholder="留空则保留当前密钥" />
        </Form.Item>

        <Form.Item name="openai_url" label="OpenAI 兼容端点">
          <Input placeholder={openaiPlaceholder} />
        </Form.Item>

        <Form.Item name="claude_url" label="Claude 兼容端点">
          <Input placeholder={claudePlaceholder} />
        </Form.Item>

        <Form.Item name="models" label="支持的模型">
          <Select
            mode="tags"
            placeholder="输入模型名称后回车"
            options={presetModels.map(m => ({ label: m, value: m }))}
          />
        </Form.Item>

        <div className="form-row">
          <Form.Item name="tpm_limit" label="TPM 限制">
            <InputNumber min={0} placeholder="0 = 不限" />
          </Form.Item>
          <Form.Item name="rpm_limit" label="RPM 限制">
            <InputNumber min={0} placeholder="0 = 不限" />
          </Form.Item>
        </div>

        <Form.Item name="source" label="来源">
          <Input placeholder="例如：linux.do" />
        </Form.Item>

        <Form.Item name="note" label="备注">
          <Input.TextArea rows={2} placeholder="备注信息（可选）" />
        </Form.Item>
      </Form>
    </Modal>
  )
}
