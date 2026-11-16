import { useCallback, useRef } from 'react'

/**
 * 分级滑块（分级档位，不是连续调节）：
 * - 轨道上每一档一个刻度点，指针只能落在档位上，读数因此永远可复述（「16px」「.03em」）。
 * - 指针 / 键盘共用同一套 index：← → 移一档，Home / End 到两端，焦点常驻在滑块本体。
 * - 交互只回传 index，值域与文案由调用方决定（字号 8–20、字距 0–0.09em 各七档）。
 */
interface Props {
  label: string
  /** 当前档位（0 起） */
  value: number
  /** 总档数（≥2） */
  steps: number
  /** 档位读数，用于滑块右侧与无障碍朗读 */
  format: (index: number) => string
  onChange: (index: number) => void
  ariaLabel: string
}

export function StepSlider({ label, value, steps, format, onChange, ariaLabel }: Props) {
  const trackRef = useRef<HTMLDivElement>(null)
  const draggingRef = useRef(false)
  const max = Math.max(0, steps - 1)
  const current = Math.min(max, Math.max(0, value))
  const ratio = max === 0 ? 0 : current / max

  const pick = useCallback(
    (clientX: number) => {
      const el = trackRef.current
      if (!el) return
      const rect = el.getBoundingClientRect()
      if (rect.width <= 0) return
      const raw = (clientX - rect.left) / rect.width
      const next = Math.min(max, Math.max(0, Math.round(raw * max)))
      if (next !== current) onChange(next)
    },
    [current, max, onChange],
  )

  const step = (delta: number) => {
    const next = Math.min(max, Math.max(0, current + delta))
    if (next !== current) onChange(next)
  }

  return (
    <div className="step-slider">
      <div className="step-slider-head">
        <span className="step-slider-label">{label}</span>
        <span className="step-slider-value" aria-hidden="true">{format(current)}</span>
      </div>
      <div
        className="step-slider-track"
        ref={trackRef}
        onPointerDown={(e) => {
          // 拖动期间不要选中文本，也不要把指针交给上层（弹层据此判断「点在外面」）
          e.preventDefault()
          e.stopPropagation()
          draggingRef.current = true
          e.currentTarget.setPointerCapture(e.pointerId)
          pick(e.clientX)
        }}
        onPointerMove={(e) => {
          if (!draggingRef.current) return
          pick(e.clientX)
        }}
        onPointerUp={(e) => {
          draggingRef.current = false
          if (e.currentTarget.hasPointerCapture(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId)
        }}
        onPointerCancel={() => { draggingRef.current = false }}
      >
        <span className="step-slider-rail" aria-hidden="true" />
        <span className="step-slider-fill" style={{ width: `${ratio * 100}%` }} aria-hidden="true" />
        {Array.from({ length: steps }, (_, i) => (
          <span
            key={i}
            className={'step-slider-tick' + (i <= current ? ' on' : '')}
            style={{ left: `${(i / max) * 100}%` }}
            aria-hidden="true"
          />
        ))}
        <span
          className="step-slider-knob"
          style={{ left: `${ratio * 100}%` }}
          role="slider"
          tabIndex={0}
          aria-label={ariaLabel}
          aria-valuemin={0}
          aria-valuemax={max}
          aria-valuenow={current}
          aria-valuetext={format(current)}
          onKeyDown={(e) => {
            if (e.key === 'ArrowRight' || e.key === 'ArrowUp') { e.preventDefault(); step(1) }
            else if (e.key === 'ArrowLeft' || e.key === 'ArrowDown') { e.preventDefault(); step(-1) }
            else if (e.key === 'Home') { e.preventDefault(); if (current !== 0) onChange(0) }
            else if (e.key === 'End') { e.preventDefault(); if (current !== max) onChange(max) }
          }}
        />
      </div>
    </div>
  )
}