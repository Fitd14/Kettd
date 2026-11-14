import { useEffect, useRef, useState } from 'react'
import type { KeyboardEvent as ReactKeyboardEvent } from 'react'
import { createPortal } from 'react-dom'

export interface LinkDialogProps {
  /** 打开时预填的链接地址（当前激活链接或空串） */
  initialUrl: string
  /** 当前是否有可应用的目标（选中文本，或光标位于链接内） */
  canApply: boolean
  /** 返回 true 表示已应用并关闭；返回 false 表示地址不可用，弹窗保留并提示 */
  onConfirm: (url: string) => boolean
  /** 移除当前链接（选中文本范围内的 link mark） */
  onRemove: () => void
  onCancel: () => void
}

const DISABLED_HINT = '先在编辑器中选中文字，或把光标放在已有链接上'
const DEFAULT_HINT = '留空确定将移除链接；Enter 确定，Esc 取消'
const INVALID_HINT = '无法应用链接：请检查地址格式（支持 http/https/mailto 等）'

/** 居中链接弹窗：地址输入 + 确定 / 取消 / 移除链接。由 TipTapEditor 挂载。 */
export function LinkDialog({
  initialUrl,
  canApply,
  onConfirm,
  onRemove,
  onCancel,
}: LinkDialogProps) {
  const [value, setValue] = useState(initialUrl)
  const [error, setError] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)
  const cancelRef = useRef<HTMLButtonElement>(null)
  const dialogRef = useRef<HTMLDivElement>(null)

  // 打开即聚焦地址输入框并全选，便于直接覆盖旧链接
  useEffect(() => {
    const input = inputRef.current
    if (input && !input.disabled) {
      input.focus()
      input.select()
    } else {
      cancelRef.current?.focus()
    }
  }, [])

  // 捕获阶段拦截 Esc：既关闭弹窗，又避免冒泡到 ProseMirror / 全局快捷键
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

  const submit = () => {
    if (!canApply) return
    if (!onConfirm(value)) {
      setError(INVALID_HINT)
      // setLink 失败时 chain().focus() 已把焦点还给编辑器，这里抢回输入框
      requestAnimationFrame(() => {
        const input = inputRef.current
        if (input) {
          input.focus()
          input.select()
        }
      })
    }
  }

  // Tab 焦点循环，保证弹窗内键盘可达
  const onDialogKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault()
      event.stopPropagation()
      onCancel()
      return
    }
    if (event.key !== 'Tab') return
    const nodes = dialogRef.current?.querySelectorAll<HTMLElement>('input, button')
    if (!nodes || nodes.length === 0) return
    const focusables = Array.from(nodes).filter((node) => !node.hasAttribute('disabled'))
    if (focusables.length === 0) return
    const first = focusables[0]
    const last = focusables[focusables.length - 1]
    const active = document.activeElement
    if (event.shiftKey && active === first) {
      event.preventDefault()
      last.focus()
    } else if (!event.shiftKey && active === last) {
      event.preventDefault()
      first.focus()
    }
  }

  return createPortal(
    <div
      className="kb-link-dialog-overlay"
      role="presentation"
      onMouseDown={(event) => {
        // 点遮罩取消；点弹窗内部不处理
        if (event.target !== event.currentTarget) return
        event.preventDefault()
        onCancel()
      }}
    >
      <div
        ref={dialogRef}
        className="kb-link-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="kb-link-dialog-title"
        onKeyDown={onDialogKeyDown}
      >
        <div className="kb-link-dialog-title" id="kb-link-dialog-title">
          链接
        </div>
        <label className="kb-link-dialog-field">
          <span className="kb-link-dialog-label">链接地址</span>
          <input
            ref={inputRef}
            className="kb-link-dialog-input"
            type="text"
            value={value}
            placeholder="https:// 或 mailto:…"
            spellCheck={false}
            autoComplete="off"
            disabled={!canApply}
            onChange={(event) => {
              setValue(event.target.value)
              if (error) setError('')
            }}
            onKeyDown={(event) => {
              if (event.key !== 'Enter' || event.nativeEvent.isComposing) return
              event.preventDefault()
              event.stopPropagation()
              submit()
            }}
          />
        </label>
        <div className={`kb-link-dialog-hint${error ? ' on-error' : ''}`}>
          {!canApply ? DISABLED_HINT : error || DEFAULT_HINT}
        </div>
        <div className="kb-link-dialog-actions">
          <button
            type="button"
            className="kb-link-dialog-btn"
            onClick={onRemove}
            disabled={!canApply}
          >
            移除链接
          </button>
          <span className="kb-link-dialog-spacer" />
          <button
            ref={cancelRef}
            type="button"
            className="kb-link-dialog-btn"
            onClick={onCancel}
          >
            取消
          </button>
          <button
            type="button"
            className="kb-link-dialog-btn kb-link-dialog-primary"
            onClick={submit}
            disabled={!canApply}
          >
            确定
          </button>
        </div>
      </div>
    </div>,
    document.body,
  )
}