import { useCallback } from 'react'
import { message } from 'antd'

export function useCopyText() {
  const copyText = useCallback(async (text: string, successMsg = '已复制到剪贴板') => {
    try {
      await navigator.clipboard.writeText(text)
      message.success(successMsg)
    } catch {
      // 降级方案
      const textArea = document.createElement('textarea')
      textArea.value = text
      textArea.style.position = 'fixed'
      textArea.style.left = '-999999px'
      document.body.appendChild(textArea)
      textArea.select()
      try {
        document.execCommand('copy')
        message.success(successMsg)
      } catch {
        message.error('复制失败，请手动复制')
      }
      document.body.removeChild(textArea)
    }
  }, [])

  return copyText
}
