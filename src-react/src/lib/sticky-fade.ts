/**
 * 便签「移出淡化」守卫（OpenSpec sticky-shell-polish D5 / tasks 4.1）。
 *
 * 旧实现只看 mouseleave 一个信号：中文输入法候选窗是独立 HWND，指针移到候选窗上
 * 会被 WebView2 误判为离开便签窗，导致打字过程中便签突然变淡。现在把判定拆成四个
 * 条件（指针已在窗口外 / 窗口已失焦 / 未在输入法组字 / 窗口内无浮层）加 250ms 去抖，
 * 全部满足才淡化；任一条件恢复都立即取消。
 *
 * 本模块保持纯函数（不读 document、不挂监听），DOM 与 React 接入在 StickyWindow.tsx，
 * 测试可直接 `node` 跑（Node 22 type stripping 可 import .ts）。
 */

/** 去抖时长：指针移出后连续 250ms 都满足条件才淡化（tasks 4.1；design D5 取值 200–300ms） */
export const FADE_DEBOUNCE_MS = 250

/** 淡化四条件；enabled = 设置开关「移出淡化」打开 且 非 mini 态 */
export interface FadeConditions {
  enabled: boolean
  pointerOutside: boolean
  hasFocus: boolean
  composing: boolean
  overlayOpen: boolean
}

/**
 * 是否允许淡化：四条件全满足才 true，任一不满足即 false。
 * enabled=false 恒 false（设置里关闭「移出淡化」或 mini 态一律不淡化）。
 */
export function shouldFade(conditions: FadeConditions): boolean {
  if (!conditions.enabled) return false
  return (
    conditions.pointerOutside &&
    !conditions.hasFocus &&
    !conditions.composing &&
    !conditions.overlayOpen
  )
}

/**
 * 指针坐标是否落在窗口外：clientX/clientY 与 innerWidth/innerHeight 逐边比较。
 * 坐标 <= 0 或 >= 宽/高视为贴边/越界（design D5 的判定口径）。
 * 非有限数（NaN / Infinity）按「窗内」处理——宁可不淡化也不误淡化。
 */
export function isPointerOutside(
  clientX: number,
  clientY: number,
  innerWidth: number,
  innerHeight: number,
): boolean {
  if (![clientX, clientY, innerWidth, innerHeight].every(Number.isFinite)) return false
  // 窗口尺寸尚未布局（宽/高为 0 或负）时无法判断，按窗内处理，避免初始化瞬间误淡化
  if (innerWidth <= 0 || innerHeight <= 0) return false
  return clientX <= 0 || clientY <= 0 || clientX >= innerWidth || clientY >= innerHeight
}