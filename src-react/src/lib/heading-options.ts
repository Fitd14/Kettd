/**
 * 标题级别下拉的纯逻辑（OpenSpec sticky-shell-polish D2 / tasks 3.1）。
 *
 * 为什么单独拆出来：选项集合、下标循环、value → 编辑命令映射都是纯数据/纯函数，
 * 组件（heading-select.tsx）与 node 直跑测试（test/heading-select.test.mjs）共用一份，
 * 避免「下拉显示七项、命令只认 H1–H4」这类两处口径分叉。
 *
 * 取值约定沿用旧原生 <select>：'p' = 正文，'1'..'6' = H1–H6 对应 level。
 * 不含 DOM / React 依赖；`ChainedCommands` 只是 type-only import，node 跑测试时被剥离。
 */
import type { ChainedCommands } from '@tiptap/core'

/** 下拉选项值：'p'（正文）或 '1'..'6'（H1–H6） */
export type HeadingValue = 'p' | '1' | '2' | '3' | '4' | '5' | '6'

/** 标题级别：0 = 正文，1..6 = H1–H6 */
export type HeadingLevel = 0 | 1 | 2 | 3 | 4 | 5 | 6

export interface HeadingOption {
  value: HeadingValue
  label: string
  level: HeadingLevel
}

/** 七项固定顺序：正文 + H1–H6（H5/H6 仅由外部 Markdown 产生，但必须能回显与降级） */
export const HEADING_OPTIONS: readonly HeadingOption[] = [
  { value: 'p', label: '正文', level: 0 },
  { value: '1', label: 'H1', level: 1 },
  { value: '2', label: 'H2', level: 2 },
  { value: '3', label: 'H3', level: 3 },
  { value: '4', label: 'H4', level: 4 },
  { value: '5', label: 'H5', level: 5 },
  { value: '6', label: 'H6', level: 6 },
]

/** value → 选项下标；未知 / 非法值一律按「正文」（0），下拉永远有可高亮项 */
export function headingIndex(value: string | null | undefined): number {
  if (typeof value !== 'string') return 0
  const index = HEADING_OPTIONS.findIndex((option) => option.value === value)
  return index >= 0 ? index : 0
}

/** value → 标题级别；'p' 与未知值返回 0（正文） */
export function toHeadingLevel(value: string | null | undefined): HeadingLevel {
  return HEADING_OPTIONS[headingIndex(value)].level
}

/** 标题级别 → value；0 / 越界 / 非数一律回 'p'（工具栏回显兜底） */
export function headingValueForLevel(level: number | null | undefined): HeadingValue {
  if (typeof level !== 'number' || !Number.isFinite(level)) return 'p'
  const rounded = Math.trunc(level)
  if (rounded < 1 || rounded > 6) return 'p'
  return String(rounded) as HeadingValue
}

/** value → 触发按钮文案（正文 / H1–H6）；未知值回「正文」 */
export function headingLabel(value: string | null | undefined): string {
  return HEADING_OPTIONS[headingIndex(value)].label
}

/**
 * 下标循环移动（↑↓ 在七项里首尾回绕）。
 * current 越界 / 非有限数先归一到合法下标；delta 可正可负、可超过一圈（取模）。
 *
 * Radix Select 用真实 DOM 焦点（roving focus）在选项间移动，↑↓ 走到端点时它自己会停住，
 * 不会回绕；组件的 ArrowDown / ArrowUp 用它把「停在端点后继续按」映射成首尾回绕（spec 键盘与指针交互）。
 */
export function nextIndex(current: number, delta: number): number {
  const length = HEADING_OPTIONS.length
  const base = Number.isFinite(current) ? ((Math.trunc(current) % length) + length) % length : 0
  const step = Number.isFinite(delta) ? Math.trunc(delta) : 0
  return (((base + step) % length) + length) % length
}

/**
 * value → 编辑命令：正文走 setParagraph；H1–H6 走 toggleHeading。
 * 未聚焦的 chain 也适用（调用方负责 .focus() 与 .run()）。
 * 再次选中当前级别时 toggleHeading 自带「取消回正文」语义，与旧下拉一致。
 */
export function applyHeadingCommand(
  chain: ChainedCommands,
  value: string | null | undefined,
): ChainedCommands {
  const level = toHeadingLevel(value)
  if (level === 0) return chain.setParagraph()
  return chain.toggleHeading({ level })
}