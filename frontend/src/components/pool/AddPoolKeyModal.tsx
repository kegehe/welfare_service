import { useState, useEffect, useMemo, useRef } from 'react'
import { Modal, Form, Input, Select, InputNumber, message } from 'antd'
import type { AddPoolKeyInput } from '@/types'
import { useModelPresets } from '@/hooks/useModelPresets'
import { PLATFORM_DEFAULTS } from '@/config/platformDefaults'

interface Props {
  open: boolean
  onClose: () => void
  onSubmit: (input: AddPoolKeyInput) => Promise<void>
}

export function AddPoolKeyModal({ open, onClose, onSubmit }: Props) {
  const [form] = Form.useForm()
  const [loading, setLoading] = useState(false)
  const [apiKeyVisible, setApiKeyVisible] = useState(true)
  const { ensurePresetsLoaded, getPresetModelsForPlatform } = useModelPresets()

  const platform = Form.useWatch('platform', form) || 'xiaomi'

  // 弹窗打开时加载预设数据
  useEffect(() => {
    if (open) ensurePresetsLoaded()
  }, [open, ensurePresetsLoaded])

  // 切换平台时自动填充 URL 和默认模型
  const platformInitRef = useRef<string | null>(null)

  useEffect(() => {
    if (!open) {
      platformInitRef.current = null
      return
    }
    // 跳过弹窗刚打开时的首次触发（此时 form 已有 initialValues）
    if (platformInitRef.current === null) {
      platformInitRef.current = platform
      return
    }
    if (platformInitRef.current === platform) return
    platformInitRef.current = platform

    const defaults = PLATFORM_DEFAULTS[platform]
    if (!defaults) return

    form.setFieldsValue({
      openai_url: defaults.openai_url,
      claude_url: defaults.claude_url,
      models: defaults.default_models.length > 0 ? defaults.default_models : [],
    })
  }, [platform, open, form])

  // 动态 placeholder
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
    try {
      const values = await form.validateFields()
      // 验证：URL 至少填一个
      if (!values.openai_url?.trim() && !values.claude_url?.trim()) {
        message.warning('OpenAI URL 和 Claude URL 至少填写一个')
        return
      }
      // 清理 models
      const modelList = (values.models || []).map((s: string) => s.trim()).filter(Boolean)
      if (modelList.length === 0) {
        message.warning('请输入至少一个模型')
        return
      }

      setLoading(true)
      await onSubmit({
        platform: values.platform,
        name: values.name?.trim() || null,
        api_key: values.api_key,
        openai_url: values.openai_url || '',
        claude_url: values.claude_url || '',
        models: modelList,
        tpm_limit: values.tpm_limit || 0,
        rpm_limit: values.rpm_limit || 0,
        source: values.source?.trim() || null,
        note: values.note?.trim() || null,
      })
      form.resetFields()
    } catch (e: any) {
      if (e.errorFields) return // 表单验证失败
      message.error('提交失败: ' + (e.message || '未知错误'))
    } finally {
      setLoading(false)
    }
  }

  return (
    <Modal
      title="添加号池 Key"
      open={open}
      onCancel={() => {
        form.resetFields()
        setApiKeyVisible(true)
        onClose()
      }}
      onOk={handleSubmit}
      confirmLoading={loading}
      width={600}
    >
      <Form
        form={form}
        layout="vertical"
        initialValues={{
          platform: 'xiaomi',
          openai_url: PLATFORM_DEFAULTS.xiaomi.openai_url,
          claude_url: PLATFORM_DEFAULTS.xiaomi.claude_url,
          models: PLATFORM_DEFAULTS.xiaomi.default_models,
        }}
      >
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

        <Form.Item name="api_key" label="API Key" rules={[{ required: true, message: '请输入 API Key' }]}>
          <Input.Password
            placeholder="sk-..."
            visibilityToggle={{ visible: apiKeyVisible, onVisibleChange: setApiKeyVisible }}
          />
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
