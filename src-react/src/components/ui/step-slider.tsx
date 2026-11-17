import { Slider as RadixSlider } from 'radix-ui'

/**
 * 分级滑块（分级档位，不是连续调节）：
 * - 指针仍能落在任意一档，读数因此永远可复述（「16px」「.03em」）。
 * - 键盘 / 指针 / 焦点圈定交给 radix `Slider`：← → / ↑ ↓ 移一档，Home / End 到两端，
 *   `aria-valuetext` 用 `format(current)`，读屏播报的是档位文案而不是裸索引。
 * - 刻度点仍自渲染：radix 的 Track 只有一条填充条，档位刻度是设计的一部分。
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
  const max = Math.max(0, steps - 1)
  const current = Math.min(max, Math.max(0, value))

  return (
    <div className="step-slider">
      <div className="step-slider-head">
        <span className="step-slider-label">{label}</span>
        <span className="step-slider-value" aria-hidden="true">{format(current)}</span>
      </div>
      <RadixSlider.Root
        className="step-slider-track"
        min={0}
        max={max}
        step={1}
        value={[current]}
        onValueChange={([next]) => onChange(next)}
      >
        <RadixSlider.Track className="step-slider-rail">
          <RadixSlider.Range className="step-slider-fill" />
        </RadixSlider.Track>
        {Array.from({ length: steps }, (_, i) => (
          <span
            key={i}
            className={'step-slider-tick' + (i <= current ? ' on' : '')}
            style={{ left: `${(i / max) * 100}%` }}
            aria-hidden="true"
          />
        ))}
        <RadixSlider.Thumb
          className="step-slider-knob"
          aria-label={ariaLabel}
          aria-valuetext={format(current)}
        />
      </RadixSlider.Root>
    </div>
  )
}