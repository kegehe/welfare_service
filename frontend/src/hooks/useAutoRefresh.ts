import { useEffect, useRef } from 'react'

export function useAutoRefresh(callback: () => Promise<void>, intervalMs = 10000) {
  const savedCallback = useRef(callback)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  useEffect(() => {
    savedCallback.current = callback
  }, [callback])

  useEffect(() => {
    const loop = async () => {
      try {
        await savedCallback.current()
      } catch {
        // 防止异常导致自动刷新停止
      }
      timerRef.current = setTimeout(loop, intervalMs)
    }

    // 首次延迟启动
    timerRef.current = setTimeout(loop, intervalMs)

    return () => {
      if (timerRef.current) {
        clearTimeout(timerRef.current)
      }
    }
  }, [intervalMs])
}
