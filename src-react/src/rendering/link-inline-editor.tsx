import { useEffect, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Popover, PopoverAnchor, PopoverContent } from '@/components/ui/popover'

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

/** 打开后的宽限：这段时间内点条外不收起，给手动输入地址留时间 */
const OUTSIDE_GRACE_MS = 3000

/**
 * 链接编辑条（kb-writing-experience D9）：点链接在它下方弹一条横向输入条，
 * 取代原来的居中模态 `LinkDialog`。非模态 —— 不锁编辑区、不遮正文，
 * 改完地址回车即走，Esc 或点别处收起。
 *
 * 浮层走 ui/popover：portal 到 body 避开祖先 overflow 裁切，贴链接下方并自动翻转，
 * 视口夹取与 Esc / 点外关闭由 radix 负责。
 */
export function LinkInlineEditor({ href, anchor, onSubmit, onRemove, onClose }: LinkInlineEditorProps) {
  const [value, setValue] = useState(href)
  const [error, setError] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)
  const openedAtRef = useRef(Date.now())

  // 打开即聚焦并全选，直接键入就替换旧地址
  useEffect(() => {
    const input = inputRef.current
    if (!input) return
    input.focus()
    input.select()
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

  // 视口夹取交给 radix 的碰撞检测；宽度固定 320，窄窗时由 max-width 收
  const width = 320

  return (
    <Popover open onOpenChange={(open) => { if (!open) onClose() }}>
      <PopoverAnchor asChild>
        <span
          aria-hidden
          style={{ position: 'fixed', left: anchor.left, top: anchor.bottom, width: 1, height: 1 }}
        />
      </PopoverAnchor>
      <PopoverContent
        bare
        side="bottom"
        align="start"
        sideOffset={6}
        className="kb-link-inline"
        style={{ width }}
        aria-label="编辑链接"
        // 保持编辑器选区与焦点：打开浮层不抢焦点，输入框自己 focus()
        onOpenAutoFocus={(event) => event.preventDefault()}
        onCloseAutoFocus={(event) => event.preventDefault()}
        // 打开后 3 秒内点条外不收起：刚点开链接爪子还在鼠标上（m01554 第 5 条）
        onPointerDownOutside={(event) => {
          if (Date.now() - openedAtRef.current < OUTSIDE_GRACE_MS) event.preventDefault()
        }}
        // 点条内不冒泡到编辑器，避免刚打开就被 handleClick 关掉
        onMouseDown={(event) => event.stopPropagation()}
      >
      <Input
        bare
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
      <Button
        variant="danger"
        size="sm"
        className="kb-link-inline-btn"
        onMouseDown={(event) => event.preventDefault()}
        onClick={onRemove}
      >
        移除
      </Button>
      <Button
        size="sm"
        className="kb-link-inline-btn"
        onMouseDown={(event) => event.preventDefault()}
        onClick={onClose}
      >
        关闭
      </Button>
      {error && <span className="kb-link-inline-err" role="alert">{error}</span>}
      </PopoverContent>
    </Popover>
  )
}