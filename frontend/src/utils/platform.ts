/**
 * Platform utilities — shared label and color mappings.
 * Eliminates duplication between ActiveKeysBar and PoolKeysTable.
 */

const PLATFORM_LABELS: Record<string, string> = {
  xiaomi: '小米',
  iflytek: '讯飞',
  anthropic: 'Anthropic',
}

const PLATFORM_COLORS_HEX: Record<string, string> = {
  xiaomi: '#ff6900',
  iflytek: '#2563eb',
  anthropic: '#f59e0b',
}

export function getPlatformLabel(platform: string): string {
  return PLATFORM_LABELS[platform] || platform || '未命名平台'
}

/** Hex color for Ant Design components that require plain color values (Tag, Badge.Ribbon) */
export function getPlatformColorHex(platform: string): string {
  return PLATFORM_COLORS_HEX[platform] || '#06b6d4'
}
