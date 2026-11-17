import { PopoverContent } from '@/components/ui/popover'
import { StepSlider } from '@/components/ui/step-slider'
import {
  READING_MEASURES,
  READING_SIZES,
  READING_TRACKINGS,
  type ReadingMeasureKey,
} from '@/lib/reading-prefs'

export interface ReadingPrefsValues {
  /** 正文字号（px，12–24 逐 px 一档） */
  size: number
  /** 字距档位索引（0 常规 / 1 宽松 / 2 较宽） */
  tracking: number
  /** 列宽上限档位 */
  measure: string
}

interface Props {
  value: ReadingPrefsValues
  /** 只回传改动的键，落库与预览走同一条 setSettings → applyReadingPrefs 链路 */
  onChange: (patch: { readingSize?: number; readingTracking?: number; readingMeasure?: string }) => void
}

/** 字号档位在 READING_SIZES 里的索引（设置值越界时回落到 15px 那一档） */
function sizeIndex(size: number): number {
  const i = (READING_SIZES as readonly number[]).indexOf(size)
  return i >= 0 ? i : (READING_SIZES as readonly number[]).indexOf(15)
}

function trackingLabel(index: number): string {
  return READING_TRACKINGS[index]?.label ?? READING_TRACKINGS[0].label
}

/**
 * 排版弹层内容（文档内，替代设置页里的三行档位）：
 * 字号 / 字距用分级滑块，列宽只有两档，用分段钮而不是滑块。
 * 开关、点外关闭、Esc 与焦点归还都由调用方的 `Popover` 承担。
 */
export function TypoPopover({ value, onChange }: Props) {
  const measure = (READING_MEASURES as readonly { key: ReadingMeasureKey; label: string; note: string }[])
    .find((m) => m.key === value.measure) ?? READING_MEASURES[0]

  return (
    <PopoverContent className="kb-typo-pop" aria-label="排版" align="end" side="bottom">
      <div className="kb-typo-head">
        <span>排版</span>
        <span className="kb-typo-note">所有条目共用</span>
      </div>

      <StepSlider
        label="字号"
        ariaLabel="正文字号"
        value={sizeIndex(value.size)}
        steps={READING_SIZES.length}
        format={(i) => `${READING_SIZES[i]}px`}
        onChange={(i) => onChange({ readingSize: READING_SIZES[i] })}
      />

      <StepSlider
        label="字距"
        ariaLabel="正文文字间距"
        value={value.tracking}
        steps={READING_TRACKINGS.length}
        format={trackingLabel}
        onChange={(i) => onChange({ readingTracking: i })}
      />

      <div className="kb-typo-measure">
        <span className="kb-typo-measure-label">列宽</span>
        <div className="kb-typo-seg" role="group" aria-label="正文列宽上限">
          {READING_MEASURES.map((m) => (
            <button
              key={m.key}
              type="button"
              className={'kb-typo-seg-btn' + (m.key === measure.key ? ' on' : '')}
              aria-pressed={m.key === measure.key}
              title={m.note}
              onClick={() => onChange({ readingMeasure: m.key })}
            >
              {m.label}
            </button>
          ))}
        </div>
      </div>
      <p className="kb-typo-foot">{measure.note}</p>
    </PopoverContent>
  )
}