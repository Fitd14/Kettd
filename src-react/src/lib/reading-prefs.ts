/**
 * 阅读偏好（字号 / 字距 / 列宽）—— 只写 `<html>` 上的三个 data-* 属性，
 * 由 `src-react/src/views/kb.css` 的局部规则消费，不碰便签窗、不碰 chrome。
 *
 * 为什么是属性而不是内联样式：知识库视图（KbView）没有 settings / bootstrap 通道，
 * 而便签窗与知识库共用 prosemirror.css 的 H4–H6 规则；把值挂在 <html> 上，
 * 再由 `.kb-detail` 子树做局部覆盖，是唯一既不引全局状态、又不污染便签窗的做法。
 *
 * 三档与 Rust 侧 `models::normalize_reading_*` 一一对应，改一处必须改两处。
 */
import type { Settings } from './api'

/** 可选字号（px）：8–20 步长 2，与 `models::READING_SIZES` 对齐 */
export const READING_SIZES = [8, 10, 12, 14, 16, 18, 20] as const

/** 字距档位换算（em）：索引 0–6 对应 0–0.09em，步长 0.015em */
export const READING_TRACKING_STEPS = [0, 0.015, 0.03, 0.045, 0.06, 0.075, 0.09] as const

/** 列宽上限两档，与 `models::READING_MEASURES` 对齐 */
export const READING_MEASURES = [
  { key: 'comfortable', label: '舒适', note: '96ch · 约 48 字/行' },
  { key: 'wide', label: '宽敞', note: '112ch · 约 56 字/行' },
] as const

export type ReadingMeasureKey = (typeof READING_MEASURES)[number]['key']

export const READING_SIZE_DEFAULT = 16
export const READING_TRACKING_DEFAULT = 0
export const READING_MEASURE_DEFAULT: ReadingMeasureKey = 'comfortable'

function pickSize(value: number | undefined): number {
  return READING_SIZES.includes(value as (typeof READING_SIZES)[number])
    ? (value as number)
    : READING_SIZE_DEFAULT
}

function pickTracking(value: number | undefined): number {
  return Number.isInteger(value) && (value as number) >= 0 && (value as number) < READING_TRACKING_STEPS.length
    ? (value as number)
    : READING_TRACKING_DEFAULT
}

function pickMeasure(value: string | undefined): ReadingMeasureKey {
  return READING_MEASURES.some((m) => m.key === value)
    ? (value as ReadingMeasureKey)
    : READING_MEASURE_DEFAULT
}

/** 字距索引 → em 字符串（设置页文案与 CSS 用同一个换算） */
export function trackingEm(index: number): string {
  return `${READING_TRACKING_STEPS[pickTracking(index)]}em`
}

/**
 * 把设置落到 `<html>`。幂等：只覆盖同名属性，不缓存、不追加第二套状态，
 * 因此启动与每次 `store-changed` 重放都安全。
 */
export function applyReadingPrefs(
  settings: Pick<Settings, 'readingSize' | 'readingTracking' | 'readingMeasure'>,
): void {
  const root = document.documentElement
  root.dataset.readingSize = String(pickSize(settings.readingSize))
  root.dataset.readingTracking = String(pickTracking(settings.readingTracking))
  root.dataset.readingMeasure = pickMeasure(settings.readingMeasure)
}