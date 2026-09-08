/**
 * 共享内核的类型化出口（ADR-0006）：快录解析与组参的唯一实现住在 src-react/src/kernel/（M4 自 vanilla src/ 迁入），
 * 全站唯一实现，禁止在 TS 侧复制第二份语义。
 */
import { parseCapture as rawParseCapture, buildCaptureArgs as rawBuildCaptureArgs } from '../kernel/capture.js'
import { localToday as rawLocalToday } from '../kernel/time.js'

export interface ParseChip {
  /** 原始子串（点击 chip 从输入里删除时用） */
  raw: string
  value: string
  kind: string
}

export interface ParsedCapture {
  title: string
  category: string | null
  priority: string | null
  dueAt: string | null
  autoRemind: boolean
  chips: ParseChip[]
  fellBack: boolean
}

export function parseCapture(text: string, today: string): ParsedCapture {
  return rawParseCapture(text, today) as ParsedCapture
}

export function buildCaptureArgs(parsed: ParsedCapture): Record<string, unknown> {
  return rawBuildCaptureArgs(parsed) as Record<string, unknown>
}

export function localToday(): string {
  return rawLocalToday() as string
}
