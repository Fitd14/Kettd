import { useEffect, useRef, useState } from 'react'
import type { KeyboardEvent as ReactKeyboardEvent } from 'react'
import { createPortal } from 'react-dom'

export interface LinkInlineEditorProps {
  /** 当前链接地址（打开时预填） */
  href: string
  /** 链接在视口里的位置：用 coordsAtPos 取，内联条贴在它下方 */
  anchor: { left: number; top: number; bottom: number }
  /** 返回 true 表示已应用并关闭；false 表示地址不合法，条留下并提示 */
  onSubmit: (url: string) => boolean
  /** 移除这条链接的 mark */
  onRemove: () => void
  /** 关闭（点外部 / Esc / 失焦） */
  onClose: () => void
}

const INVALID_HINT = '地址要 http / https / mailto 开头'

/**
 * 链接编辑条（kb-writing-experience D9）：点链接在它下方弹一条横向输入条，
 * 取代原来的居中模态 `LinkDialog`。非模态 —— 不锁编辑区、不遮正文，
 * 改完地址回车即走，Esc 或点别处收起。
 *
 * portal 到 body：知识库详情面板与便签窄窗都可能被祖先的 overflow 裁切。
 */
export function LinkInlineEditor({ href, anchor, onSubmit, onRemove, onClose }: LinkInlineEditorProps) {
  const [value, setValue] = useState(href)
  const [error, setError] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)
  const barRef = useRef<HTMLDivElement>(null)
  const closeRef = useRef(onClose)
  closeRef.current = onClose

  // 打开即聚焦并全选，直接键入就替换旧地址
  useEffect(() => {
    const input = inputRef.current
    if (!input) return
    input.focus()
    input.select()
  }, [])

  // 点条外任意处收起（不吞事件：先关条，那次点击照常生效）
  useEffect(() => {
    const onDocPointerDown = (event: MouseEvent) => {
      const bar = barRef.current
      if (bar && event.target instanceof Node && bar.contains(event.target)) return
      closeRef.current()
    }
    document.addEventListener('mousedown', onDocPointerDown, true)
    return () => document.removeEventListener('mousedown', onDocPointerDown, true)
  }, [])

  const submit = () => {
    if (onSubmit(value)) return
    setError(INVALID_HINT)
    requestAnimationFrame(() => {
      const input = inputRef.current
      if (input) {
        input.focus()
        input.select()
      }
    })
  }

  const onKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault()
      event.stopPropagation()
      onClose()
    }
  }

  // 视口内夹住：链接贴近右/下边缘时条不越界
  const width = 320
  const left = Math.max(8, Math.min(anchor.left, window.innerWidth - width - 8))
  const top = Math.min(anchor.bottom + 6, window.innerHeight - 56)

  return createPortal(
    <div
      ref={barRef}
      className="kb-link-inline"
      style={{ left, top, width }}
      role="dialog"
      aria-label="编辑链接"
      onKeyDown={onKeyDown}
      // 点条内不冒泡到编辑器，避免刚打开就被 handleClick 关掉
      onMouseDown={(event) => event.stopPropagation()}
    >
      <input
        ref={inputRef}
        className="kb-link-inline-input"
        type="text"
        value={value}
        placeholder="https:// 或 mailto:…"
        spellCheck={false}
        autoComplete="off"
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
      <button
        type="button"
        className="kb-link-inline-btn danger"
        onMouseDown={(event) => event.preventDefault()}
        onClick={onRemove}
      >
        移除
      </button>
      <button
        type="button"
        className="kb-link-inline-btn"
        onMouseDown={(event) => event.preventDefault()}
        onClick={onClose}
      >
        关闭
      </button>
      {error && <span className="kb-link-inline-err" role="alert">{error}</span>}
    </div>,
    document.body,
  )
}