import { Select as RadixSelect } from 'radix-ui'
import { clsx } from 'clsx'
import { ChevronDown } from 'lucide-react'

export interface SelectOption {
  value: string
  label: string
}

interface Props {
  value: string
  onValueChange: (value: string) => void
  options: ReadonlyArray<SelectOption>
  ariaLabel: string
  className?: string
  /** 空字符串不能作为 radix 的 item value，映射成这个哨兵值 */
  emptyValue?: string
}

/**
 * 下拉选择原语（radix Select 承载行为，样式走 primitives.css 的 `.select-*`）。
 * 空字符串用 `emptyValue` 哨兵替换，回调仍传回空串，调用方不必关心 radix 的限制。
 */
export function Select({ value, onValueChange, options, ariaLabel, className, emptyValue = '__empty__' }: Props) {
  const toRadix = (raw: string) => (raw === '' ? emptyValue : raw)
  const fromRadix = (raw: string) => (raw === emptyValue ? '' : raw)

  return (
    <RadixSelect.Root value={toRadix(value)} onValueChange={(next) => onValueChange(fromRadix(next))}>
      <RadixSelect.Trigger className={clsx('input', 'select-trigger', className)} aria-label={ariaLabel}>
        <RadixSelect.Value />
        <RadixSelect.Icon className="select-caret" aria-hidden="true">
          <ChevronDown size={12} strokeWidth={1.8} />
        </RadixSelect.Icon>
      </RadixSelect.Trigger>
      <RadixSelect.Portal>
        <RadixSelect.Content className="select-content" position="popper" sideOffset={4}>
          <RadixSelect.Viewport className="select-viewport">
            {options.map((option) => (
              <RadixSelect.Item key={option.value} value={toRadix(option.value)} className="select-item">
                <RadixSelect.ItemText>{option.label}</RadixSelect.ItemText>
              </RadixSelect.Item>
            ))}
          </RadixSelect.Viewport>
        </RadixSelect.Content>
      </RadixSelect.Portal>
    </RadixSelect.Root>
  )
}