/**
 * Welfare Service — 共享主题配置
 * 「信号协议」Signal Protocol — 与 CSS 变量保持同步
 */

import type { ThemeConfig } from 'antd'

export const COLORS = {
  pool: '#06b6d4',        // backward compat alias for flow
  flow: '#06b6d4',
  flowLight: '#22d3ee',
  flowDark: '#0891b2',
  trace: '#8b5cf6',
  traceLight: '#a78bfa',
  lime: '#84cc16',
  signal: '#22c55e',
  fuse: '#f59e0b',
  fault: '#ef4444',
  info: '#64748b',
  gridEmpty: '#e2e8f0',
  border: '#e2e8f0',
  conduit: '#1e293b',
  systemBlue: '#3b82f6',
} as const

export const antdTheme: ThemeConfig = {
  token: {
    colorPrimary: COLORS.flow,
    colorSuccess: COLORS.signal,
    colorWarning: COLORS.fuse,
    colorError: COLORS.fault,
    colorInfo: COLORS.info,
    borderRadius: 8,
    colorBorder: '#e2e8f0',
    colorBgContainer: '#ffffff',
    colorBgLayout: '#f8fafb',
    colorText: '#0f172a',
    colorTextSecondary: '#475569',
    fontFamily:
      '"Inter", -apple-system, BlinkMacSystemFont, "Segoe UI", "Noto Sans SC", Roboto, "Helvetica Neue", sans-serif',
    fontFamilyCode:
      '"SF Mono", "JetBrains Mono", "Fira Code", "Cascadia Code", ui-monospace, monospace',
    fontSize: 14,
    lineHeight: 1.6,
    controlHeight: 34,
  },
  components: {
    Button: {
      borderRadius: 8,
      fontWeight: 500,
      primaryShadow: '0 2px 6px rgba(6, 182, 212, 0.3)',
    },
    Card: {
      borderRadius: 12,
      paddingLG: 24,
      boxShadow: '0 1px 3px rgba(0,0,0,0.04), 0 1px 2px rgba(0,0,0,0.06)',
    },
    Modal: {
      borderRadius: 16,
      titleFontSize: 16,
    },
    Table: {
      headerBg: '#f1f5f9',
      headerColor: '#475569',
      rowHoverBg: 'rgba(6, 182, 212, 0.03)',
    },
    Tag: {
      borderRadiusSM: 6,
    },
    Segmented: {
      itemSelectedBg: '#ffffff',
      trackBg: '#f1f5f9',
    },
  },
}
