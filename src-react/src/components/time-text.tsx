import { useEffect, useState } from 'react'

const HHMM = /^([01]\d|2[0-3]):[0-5]\d$/

interface Props {
  value: string
  /** 提交（合法时）；失焦或 Enter 触发 */
  onCommit: (value: string) => void
  /** 允许多时刻 HH:MM/HH:MM（循环提醒，V2-API 语义） */
  allowMulti?: boolean
  ariaLabel: string
  placeholder?: string
}

/**
 * HH:MM 时刻输入（planned-settings-ui-spec B.4/A.3 的「时刻选择器」自绘实现）：
 * 替代原生 type=time 的堆叠；非法值标红且不提交，失焦/回车提交。
 */
export function TimeText({ value, onCommit, allowMulti, ariaLabel, placeholder }: Props) {
  const [text, setText] = useState(value)
  const [bad, setBad] = useState(false)

  useEffect(() => { setText(value) }, [value])

  const tryCommit = () => {
    const v = text.trim()
    const parts = v.split('/')
    const ok = parts.every((p) => HHMM.test(p.trim())) && (allowMulti || parts.length === 1)
    if (!ok) { setBad(true); return }
    setBad(false)
    onCommit(parts.map((p) => p.trim()).join('/'))
  }

  return (
    <span className="row-flex items-center gap-1">
      <input
        className={`input xs mono time-text${bad ? ' bad' : ''}`}
        value={text}
        placeholder={placeholder ?? (allowMulti ? '09:30 或 09:30/18:00' : '09:30')}
        aria-label={ariaLabel}
        aria-invalid={bad}
        onChange={(e) => { setBad(false); setText(e.target.value) }}
        onBlur={tryCommit}
        onKeyDown={(e) => { if (e.key === 'Enter') { e.preventDefault(); tryCommit() } }}
      />
      {bad && (
        <span className="tiny" role="alert" style={{ color: 'hsl(var(--destructive))' }}>
          格式应为 {allowMulti ? '09:30 或 09:30/18:00' : '09:30'}
        </span>
      )}
    </span>
  )
}
