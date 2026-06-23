import { ElMessage } from 'element-plus'

function fallbackCopyText(text: string): boolean {
  if (typeof document === 'undefined') return false

  const textarea = document.createElement('textarea')
  textarea.value = text
  textarea.setAttribute('readonly', '')
  textarea.style.position = 'fixed'
  textarea.style.top = '0'
  textarea.style.left = '0'
  textarea.style.width = '1px'
  textarea.style.height = '1px'
  textarea.style.padding = '0'
  textarea.style.border = '0'
  textarea.style.outline = '0'
  textarea.style.boxShadow = 'none'
  textarea.style.opacity = '0'

  const activeElement = document.activeElement instanceof HTMLElement
    ? document.activeElement
    : null
  const selection = document.getSelection()
  const selectedRanges: Range[] = []
  if (selection) {
    for (let index = 0; index < selection.rangeCount; index += 1) {
      selectedRanges.push(selection.getRangeAt(index))
    }
  }

  document.body.appendChild(textarea)
  textarea.focus({ preventScroll: true })
  textarea.select()
  textarea.setSelectionRange(0, text.length)

  try {
    return document.execCommand('copy')
  } catch {
    return false
  } finally {
    textarea.remove()
    if (selection) {
      selection.removeAllRanges()
      selectedRanges.forEach(range => selection.addRange(range))
    }
    activeElement?.focus({ preventScroll: true })
  }
}

export async function copyToClipboard(text: string): Promise<void> {
  if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text)
      return
    } catch {
      // Fall back for insecure origins, embedded pages, and denied clipboard permissions.
    }
  }

  if (fallbackCopyText(text)) return

  throw new Error('clipboard copy failed')
}

export async function copyTextWithMessage(text: string, successMessage = '已复制到剪贴板') {
  try {
    await copyToClipboard(text)
    ElMessage.success(successMessage)
  } catch {
    ElMessage.error('复制失败，请手动选择文本复制')
  }
}
