export interface PlatformDefault {
  openai_url: string
  claude_url: string
  default_models: string[]
}

export const PLATFORM_DEFAULTS: Record<string, PlatformDefault> = {
  xiaomi: {
    openai_url: 'https://token-plan-cn.xiaomimimo.com/v1',
    claude_url: 'https://token-plan-cn.xiaomimimo.com/anthropic',
    default_models: ['mimo-v2.5-pro'],
  },
  iflytek: {
    openai_url: 'https://maas-coding-api.cn-huabei-1.xf-yun.com/v2',
    claude_url: 'https://maas-coding-api.cn-huabei-1.xf-yun.com/anthropic',
    default_models: ['4.0Ultra'],
  },
  anthropic: {
    openai_url: '',
    claude_url: '',
    default_models: [],
  },
}
