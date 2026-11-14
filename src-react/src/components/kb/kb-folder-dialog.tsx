import { useCallback, useEffect, useRef, useState } from 'react'
import type { KeyboardEvent as ReactKeyboardEvent } from 'react'
import { createPortal } from 'react-dom'

export interface KbFolderDialogProps {
  /** 弹窗标题（新建文件夹 / 重命名文件夹） */
  title: string
  /** 名称初始值（重命名预填；新建传空） */
  initialName?: string
  /** 提交按钮文案 */
  submitLabel?: string
  /**
   * 提交回调：成功返回 null（父级关闭弹窗），失败返回后端文案（弹窗内联显示，不关闭）。
   */
  onSubmit: (name: string) => Promise<string | null>
  onCancel: () => void
}

/**
 * 文件夹新建 / 重命名弹窗（任务 #4）。
 * 交互沿用 LinkDialog：createPortal 到 body、打开即聚焦全选、Enter 提交、Esc 取消、
 * Tab 焦点循环、点击遮罩取消；错误内联显示，成功后由父级卸载（本组件不自行关闭）。
 */
export function KbFolderDialog({
  title,
  initialName = '',
  submitLabel = '确定',
  onSubmit,
  onCancel,
}: KbFolderDialogProps) {
  const [value, setValue] = useState(initialName)
  const [error, setError] = useState<string | null>(null)
  const [submitting, setSubmitting] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)
  const dialogRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const input = inputRef.current
    if (input) {
      input.focus()
      input.select()
    }
  }, [])

  // Esc 取消：捕获阶段监听，先于编辑器 / 全局快捷键处理
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' || event.isComposing) return
      event.preventDefault()
      event.stopPropagation()
      onCancel()
    }
    document.addEventListener('keydown', onKeyDown, true)
    return () => document.removeEventListener('keydown', onKeyDown, true)
  }, [onCancel])

  const submit = useCallback(async () => {
    if (submitting) return
    const name = value.trim()
    if (!name) {
      setError('文件夹名不能为空')
      return
    }
    setSubmitting(true)
    setError(null)
    let message: string | null
    try {
      message = await onSubmit(name)
    } catch (e) {
      message = e instanceof Error ? e.message : String(e)
    }
    // 成功时父级会卸载弹窗，这里不再 setState（避免卸载后更新）
    if (message) {
      setError(message)
      setSubmitting(false)
    }
  }, [onSubmit, submitting, value])

  const handleInputKeyDown = (event: ReactKeyboardEvent<HTMLInputElement>) => {
    if (event.key !== 'Enter' || event.nativeEvent.isComposing) return
    event.preventDefault()
    void submit()
  }

  // Tab 焦点循环（与 LinkDialog 一致：首尾回环，跳过 disabled）
  const handleDialogKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key !== 'Tab') return
    const dialog = dialogRef.current
    if (!dialog) return
    const focusables = Array.from(
      dialog.querySelectorAll<HTMLElement>('input, button'),
    ).filter((el) => !el.hasAttribute('disabled'))
    if (focusables.length === 0) return
    const first = focusables[0]
    const last = focusables[focusables.length - 1]
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault()
      last.focus()
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault()
      first.focus()
    }
  }

  return createPortal(
    <div
      className="kb-folder-dialog-overlay"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !submitting) onCancel()
      }}
    >
      <div
        className="kb-folder-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        ref={dialogRef}
        onKeyDown={handleDialogKeyDown}
      >
        <div className="kb-folder-dialog-title">{title}</div>
        <label className="kb-folder-dialog-label" htmlFor="kb-folder-name">
          文件夹名
        </label>
        <input
          id="kb-folder-name"
          ref={inputRef}
          className="kb-folder-dialog-input"
          value={value}
          disabled={submitting}
          placeholder="例如：会议纪要"
          onChange={(event) => {
            setValue(event.target.value)
            if (error) setError(null)
          }}
          onKeyDown={handleInputKeyDown}
        />
        <div className="kb-folder-dialog-error" role={error ? 'alert' : undefined}>
          {error ?? ''}
        </div>
        <div className="kb-folder-dialog-actions">
          <button
            type="button"
            className="btn xs ghost"
            onClick={onCancel}
            disabled={submitting}
          >
            取消
          </button>
          <button
            type="button"
            className="btn xs primary"
            onClick={() => void submit()}
            disabled={submitting}
          >
            {submitting ? '处理中…' : submitLabel}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  )
}