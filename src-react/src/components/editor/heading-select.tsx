import { useCallback, useRef, useState } from 'react'
import type { KeyboardEvent as ReactKeyboardEvent } from 'react'
import { Select } from 'radix-ui'
import { Check, ChevronDown } from 'lucide-react'
import {
  HEADING_OPTIONS,
  headingIndex,
  headingLabel,
  nextIndex,
  type HeadingValue,
} from '@/lib/heading-options'

/**
 * 标题级别下拉（OpenSpec sticky-shell-polish D2 / tasks 3.2–3.6，用户 2026-11-15 确认改用 Radix Select）。
 *
 * - 浮层由 `radix-ui` 的 `Select.Content`（`position="popper"`）渲染：应用自绘 DOM，
 *   主题 token 全由 `prosemirror.css` 的 `.kb-editor-heading-*` 决定，暗色主题不会出现 OS 白底列表；
 *   定位交给 floating-ui（`sideOffset` 4、`collisionPadding` 8，视口内自动翻转 / 夹取，可用高度走
 *   `--radix-select-content-available-height`），组件自己不算矩形、不写内联位置；
 * - 层级：`Content` 自身 `z-index: 121`，用 portal 挂到 `document.body`。高于知识库右键菜单（120）
 *   与详情弹窗（100），且在便签窗 1200px 窄容器里页内变高时也不会被后弹的弹窗压住；
 * - 键盘：Radix 用真实 DOM 焦点在选项间移动（roving focus，`←→` 只在 H1–H6 分档时生效）；
 *   自带的 ↑↓ 走到端点会停住，本组件的 `ArrowDown` / `ArrowUp` 包一层，把「停在端点后继续按」
 *   映射成首尾回绕（spec「方向键循环」）；Enter / Space 应用、Esc 关闭并还焦触发按钮、
 *   Tab 由 Radix 拦下不关闭浮层（design D2 已改写此口径）；带修饰键的快捷键（如 Ctrl+F）放行；
 * - 选区：组件不自己拦 mousedown / pointerdown —— radix 在选项上登记的是 pointerup 选中，
 *   在 pointerdown 上 preventDefault 会让浏览器取消后续 mousedown、pointerup 的选区判断失效，
 *   点选项反而不生效。radix 自身已在 Trigger 的 pointerdown 与 Item 的 pointermove 上用
 *   preventScroll 聚焦，不折叠编辑器选区；关闭时下一帧检查浮层里是否还有焦点，没有就还焦编辑器；
 * - 只有 `onSelect` 向上抛 value，真正的编辑器命令由父组件执行（本组件不持有 editor）。
 */
export interface HeadingSelectProps {
  /** 当前光标所在块的级别：'p'（正文）或 '1'..'6'（H1–H6），未知值按正文回显 */
  value: string
  /** 选中某项：由父组件执行「正文 → setParagraph / H1–H6 → toggleHeading」 */
  onSelect: (value: HeadingValue) => void
  /** 禁用态：不打开下拉、不响应键盘 */
  disabled?: boolean
  /** 触发按钮的无障碍名称（默认「标题级别」） */
  ariaLabel?: string
}

/** 只有组合键放行：不拦 Ctrl / Alt / Meta 系快捷键（编辑器焦点由父组件还原） */
function isModified(event: ReactKeyboardEvent): boolean {
  return event.ctrlKey || event.metaKey || event.altKey
}

export function HeadingSelect({
  value,
  onSelect,
  disabled = false,
  ariaLabel = '标题级别',
}: HeadingSelectProps) {
  const contentRef = useRef<HTMLDivElement>(null)
  const [open, setOpen] = useState(false)

  const currentLabel = headingLabel(value)
  // 当前块是标题（H1–H6）时沿用旧 .on 高亮；正文不加
  const isHeading = headingIndex(value) > 0

  /** 把焦点交给最可能装着光标的元素（聚焦后，传进来的 visibleDOM 就是那个元素） */
  const restoreFocus = useCallback((visibleDOM: HTMLElement) => {
    const input = visibleDOM.querySelector<HTMLInputElement>('input, textarea')
    const editable = visibleDOM.querySelector<HTMLElement>('[contenteditable="true"]')
    ;(input ?? editable ?? visibleDOM).focus({ preventScroll: true })
  }, [])

  const handleContentKeyDown = useCallback((event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (isModified(event)) return
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return
    const items = contentRef.current?.querySelectorAll<HTMLElement>('[role="option"]')
    if (!items || items.length === 0) return
    const index = Array.prototype.indexOf.call(items, document.activeElement)
    if (index < 0) return
    const last = items.length - 1
    const isEdge = event.key === 'ArrowDown' ? index === last : index === 0
    // 非端点交给 Radix 自己移动焦点；端点交给 Radix 只会停住，这里改走回绕
    if (!isEdge) return
    event.preventDefault()
    items[nextIndex(index, event.key === 'ArrowDown' ? 1 : -1)]?.focus()
  }, [])

  // 选区与焦点：不自己拦 mousedown / pointerdown —— radix 先在选项上登记了
// pointerup 选中，若在 pointerdown 上 preventDefault，浏览器会取消后续 mousedown，
// pointerup 的选区判断随之失效，点选项反而不生效。radix 自身已在 Trigger 的 pointerdown
// 与 Item 的 pointermove 上用 preventScroll 聚焦，不折叠编辑器选区；本组件不额外抢焦点。

  return (
    <Select.Root
      value={value}
      open={open}
      onOpenChange={setOpen}
      disabled={disabled}
      onValueChange={(next) => {
        const option = HEADING_OPTIONS[headingIndex(next)]
        if (!option) return
        onSelect(option.value)
      }}
    >
      <Select.Trigger
        type="button"
        className={'kb-editor-heading' + (isHeading ? ' on' : '')}
        title={ariaLabel}
        aria-label={`${ariaLabel}：${currentLabel}`}
      >
        <Select.Value>{currentLabel}</Select.Value>
        <ChevronDown size={10} strokeWidth={2} className="kb-editor-heading-caret" aria-hidden="true" />
      </Select.Trigger>

      <Select.Portal>
        <Select.Content
          ref={contentRef}
          className="kb-editor-heading-menu"
          position="popper"
          side="bottom"
          align="start"
          sideOffset={4}
          collisionPadding={8}
          aria-label={ariaLabel}
          onKeyDown={handleContentKeyDown}
          onCloseAutoFocus={(event) => {
            // 默认行为是回到触发按钮：点选 / Enter / Esc 时保留（spec「Esc 关闭并还焦」）。
            // Tab 不会关闭浮层（Radix 拦下 Tab 键），但焦点会离开选项；下一帧检查
            // 「浮层里已经没有焦点」就把编辑器焦点还回去，否则正文明明有选区却敲不进字。
            const target = event.currentTarget
            if (!(target instanceof HTMLElement)) return
            window.setTimeout(() => {
              const active = document.activeElement
              if (active instanceof Node && target.contains(active)) return
              restoreFocus(target)
            }, 0)
          }}
        >
          <Select.Viewport className="kb-editor-heading-viewport">
            {HEADING_OPTIONS.map((option) => (
              <Select.Item
                key={option.value}
                value={option.value}
                className="kb-editor-heading-option"
              >
                <span className="kb-editor-heading-option-label">{option.label}</span>
                <Select.ItemIndicator className="kb-editor-heading-check">
                  <Check size={12} strokeWidth={2} aria-hidden="true" />
                </Select.ItemIndicator>
              </Select.Item>
            ))}
          </Select.Viewport>
        </Select.Content>
      </Select.Portal>
    </Select.Root>
  )
}