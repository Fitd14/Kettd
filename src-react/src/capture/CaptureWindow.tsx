import { useCallback, useEffect, useRef, useState } from 'react'
import { buildCaptureArgs, localToday, parseCapture, type ParsedCapture } from '@/lib/kernel'
import { call, onEvent } from '@/lib/bridge'
import { addTask } from '@/lib/api'
import './capture.css'

interface TaskRow {
  id: string
  title: string
}

type Phase = 'input' | 'receipt'

/**
 * 快速记录条（M1 · ADR-0001 D5 最小试点）。
 * 行为契约与 vanilla src/capture.html 逐条对齐：
 * 回车落库（≤2s 收件箱可见）· chips 可点删 · 纯文本兜底 · 失焦关闭 · 已保存后 Esc/回车收起。
 * 材质走 --vellum-* 令牌（styles.css/index.css 单一来源）。
 */
export default function CaptureWindow() {
  const [text, setText] = useState('')
  const [phase, setPhase] = useState<Phase>('input')
  const [saved, setSaved] = useState<TaskRow | null>(null)
  const [receiptNote, setReceiptNote] = useState('2s 后自动收起')
  const [error, setError] = useState<string | null>(null)
  const inputRef = useRef<HTMLInputElement>(null)
  const frameRef = useRef<HTMLDivElement>(null)
  const savedRef = useRef<TaskRow | null>(null)
  const closingRef = useRef(false)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  // 已上报给后端的窗口高度；与实测差 ≥1px 才再报，避免 resize 回环
  const sentHeightRef = useRef(0)

  const trimmed = text.trim()
  const parsed: ParsedCapture | null = trimmed ? parseCapture(trimmed, localToday()) : null

  const clearTimer = () => {
    if (timerRef.current) {
      clearTimeout(timerRef.current)
      timerRef.current = null
    }
  }

  const reset = useCallback(() => {
    setText('')
    setSaved(null)
    savedRef.current = null
    setPhase('input')
    setError(null)
  }, [])

  const closeOverlay = useCallback(async () => {
    if (closingRef.current) return
    closingRef.current = true
    await call('close_capture_overlay')
    // 窗隐藏后重置，落库回执不得在下次唤起时残留
    setTimeout(() => {
      closingRef.current = false
      reset()
      clearTimer()
    }, 80)
  }, [reset])

  const commit = async () => {
    const raw = text.trim()
    if (!raw) {
      setError('标题不能为空')
      return
    }
    const payload = buildCaptureArgs(parseCapture(raw, localToday()))
    // 走 api.ts 封装（自动包 { args }）——M1 试点曾裸传 payload 导致
    // 「command add_task missing required key args」（真机 2026-09-08 捕获条复现）
    const r = await addTask(payload)
    if (r.err || !r.data) {
      setError((r.err ?? '保存失败') + ' · 输入已保留，改完再按回车')
      return
    }
    setSaved(r.data)
    savedRef.current = r.data
    setPhase('receipt')
    setReceiptNote('2s 后自动收起')
    clearTimer()
    timerRef.current = setTimeout(() => { void closeOverlay() }, 2000)
  }

  const planForToday = async () => {
    const target = savedRef.current
    if (!target) {
      void closeOverlay()
      return
    }
    const r = await call('update_task', { id: target.id, patch: { plannedDate: localToday() } })
    if (r.err) {
      setReceiptNote(r.err)
      return
    }
    setReceiptNote('已加入今天 ✓')
    clearTimer()
    timerRef.current = setTimeout(() => { void closeOverlay() }, 900)
  }

  // chip 点击：从输入里删掉对应原始子串（等价 vanilla 的 replace + 空白折叠）
  const removeChip = (raw: string) => {
    setText((current) => current.replace(raw, ' ').replace(/\s+/g, ' ').trim())
    requestAnimationFrame(() => inputRef.current?.focus())
  }

  // 失焦自动收起（落库回执期间除外 —— 用户可能正要点「加入今天」）
  useEffect(() => {
    const onBlur = () => { if (!savedRef.current) void closeOverlay() }
    window.addEventListener('blur', onBlur)
    return () => window.removeEventListener('blur', onBlur)
  }, [closeOverlay])

  // 真·动态高度（方案 B）：内容实高（空输入≈45 / 带 chips≈68 / 回执≈45）上报后端改窗口高，
  // 顶边锚定向下伸缩。宽度恒 560 不参与；触发时机覆盖 chips 出没、错误行、回执切换、
  // capture-opened 重置（reset 发生在窗隐藏后 80ms，改高在隐藏期完成，下次唤起无跳变）。
  useEffect(() => {
    const el = frameRef.current
    if (!el || typeof ResizeObserver === 'undefined') return
    const ro = new ResizeObserver(() => {
      // ceil 防亚像素裁切；后端 sanitize 兜底钳制（36..200）
      const h = Math.ceil(el.getBoundingClientRect().height)
      if (h > 0 && Math.abs(h - sentHeightRef.current) >= 1) {
        sentHeightRef.current = h
        void call('capture_resize', { height: h })
      }
    })
    ro.observe(el)
    return () => ro.disconnect()
  }, [])

  // 后端事件：capture-opened 重置并聚焦；theme-changed 翻暗色
  useEffect(() => {
    const offOpened = onEvent('capture-opened', () => {
      clearTimer()
      reset()
      setTimeout(() => inputRef.current?.focus(), 30)
    })
    const offTheme = onEvent('theme-changed', (payload) => {
      document.documentElement.classList.toggle('dark', payload === 'dark')
    })
    return () => { offOpened(); offTheme() }
  }, [reset])

  // 首挂：取当前主题 + 聚焦（窗口未显示时拒焦则忽略）
  useEffect(() => {
    void (async () => {
      const s = await call<{ theme?: string }>('get_settings')
      if (!s.err && s.data?.theme) {
        document.documentElement.classList.toggle('dark', s.data.theme === 'dark')
      }
      setTimeout(() => { try { inputRef.current?.focus() } catch { /* 拒焦则忽略 */ } }, 60)
    })()
    return clearTimer
  }, [])

  const onInputKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      if (savedRef.current) { void closeOverlay(); return }
      void commit()
    }
    if (e.key === 'Escape') {
      e.preventDefault()
      void closeOverlay()
    }
  }

  return (
    <div className="cap-frame" id="frame" ref={frameRef}>
      {phase === 'input' ? (
        <div className="cap-row">
          <span className="cap-hint" aria-hidden data-tauri-drag-region title="按住拖动">✎</span>
          <input
            ref={inputRef}
            className={`input${error ? ' invalid' : ''}`}
            value={text}
            placeholder="记一条…"
            maxLength={200}
            autoFocus
            onChange={(e) => { setText(e.target.value); setError(null) }}
            onKeyDown={onInputKeyDown}
          />
          <button className="btn icon" title="记下（Enter）" aria-label="记下" onClick={() => { void commit() }}>
            <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14" /></svg>
          </button>
          <button className="btn ghost xs" title="关闭（Esc）" aria-label="关闭" onClick={() => { void closeOverlay() }}>✕</button>
        </div>
      ) : (
        <div className="cap-receipt">
          <span className="receipt-title">已记下 ·「{saved?.title}」</span>
          <button className="btn xs outline" onClick={() => { void planForToday() }}>加入今天?</button>
          <span className="tiny muted">{receiptNote}</span>
        </div>
      )}

      {phase === 'input' && parsed && parsed.chips.length > 0 && (
        <div className="cap-chips">
          {parsed.chips.map((chip) => (
            <button
              key={chip.raw}
              type="button"
              className="chip"
              title={`点击移除 ${chip.raw}`}
              onClick={() => removeChip(chip.raw)}
            >
              {chip.value}
              <span className="x" aria-hidden>×</span>
            </button>
          ))}
          {parsed.autoRemind && <span className="badge">将随截止自动提醒</span>}
        </div>
      )}
      {phase === 'input' && trimmed && parsed?.fellBack && parsed.chips.length === 0 && (
        <div className="cap-chips">
          <span className="badge">纯文本 · 落收件箱</span>
        </div>
      )}

      {error && <div className="inline-err" role="alert">{error}</div>}
    </div>
  )
}
