import { onMounted, onUnmounted } from 'vue'

export function useAutoRefresh(callback: () => Promise<void>, intervalMs = 10000) {
  let timer: ReturnType<typeof setTimeout> | null = null

  const loop = async () => {
    try {
      await callback()
    } catch {
      // callback 内部应自行处理错误，此处仅防止异常导致自动刷新停止
    }
    timer = setTimeout(loop, intervalMs)
  }

  const start = () => {
    stop()
    timer = setTimeout(loop, intervalMs)
  }

  const stop = () => {
    if (timer) {
      clearTimeout(timer)
      timer = null
    }
  }

  onMounted(start)
  onUnmounted(stop)

  return { start, stop }
}
