import { computed } from 'vue'

/**
 * 计算用户实际访问服务的 base_url。
 *
 * 策略：优先使用浏览器当前地址（因为管理面板通常通过反向代理访问，
 * 浏览器地址就是用户实际可用的地址），回退到后端 health API 返回的 base_url，
 * 最后回退到默认值。
 *
 * - Claude Code: effectiveBaseUrl（不带 /v1，Claude Code 自动拼接 /v1/messages）
 * - Codex: effectiveBaseUrl + /v1（Codex/OpenAI SDK 自动拼接 /chat/completions）
 */
export function useEffectiveBaseUrl(backendBaseUrl: () => string) {
  const effectiveBaseUrl = computed(() => {
    if (typeof window !== 'undefined') {
      const { protocol, hostname, port } = window.location
      // 标准 HTTP(80) / HTTPS(443) 端口时 window.location.port 为空字符串
      const isDefaultPort = (protocol === 'https:' && port === '') ||
                            (protocol === 'http:' && port === '')
      const portStr = (!port || isDefaultPort) ? '' : `:${port}`
      return `${protocol}//${hostname}${portStr}`
    }
    // SSR 或非浏览器环境：使用后端返回的 base_url
    return backendBaseUrl() || 'http://127.0.0.1:8080'
  })

  const claudeBaseUrl = computed(() => effectiveBaseUrl.value)
  const codexBaseUrl = computed(() => `${effectiveBaseUrl.value}/v1`)

  return { effectiveBaseUrl, claudeBaseUrl, codexBaseUrl }
}
