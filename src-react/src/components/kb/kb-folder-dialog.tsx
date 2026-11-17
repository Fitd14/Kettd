import { useCallback, useEffect, useRef, useState } from 'react'
import type { KeyboardEvent as ReactKeyboardEvent } from 'react'
import { Dialog, DialogContent, DialogTitle } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'

export interface KbFolderDialogProps {
  /** 弹窗标题（新建文件夹 / 重命名文件夹） */
  title: string
  /** 名称初始值（重命名预填；新建传空） */
  initialName?: string
  /** 提交按钮文案 */
  submitLabel?: string
  /**
   * 提交回调：成功返回 null（父级关闭弹窗），失败返回后端文案（弹窗内联显示，不关闭）。
   */
  onSubmit: (name: string) => Promise<string | null>
  onCancel: () => void
}

/**
 * 文件夹新建 / 重命名弹窗（任务 #4）。
 * 结构交给 `components/ui/dialog`（radix）：portal、遮罩、Esc、焦点圈定与归还都由原语承担；
 * 本组件只保留业务：打开即聚焦全选、Enter 提交、错误内联、提交中禁用按钮。
 */
export function KbFolderDialog({
  title,
  initialName = '',
  submitLabel = '确定',
  onSubmit,
  onCancel,
}: KbFolderDialogProps) {
  const [value, setValue] = useState(initialName)
  const [error, setError] = useState<string | null>(null)
  const [submitting, setSubmitting] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)
  const returnFocusRef = useRef<HTMLElement | null>(null)

  // 打开前记住触发元素，关闭时 radix 的 onCloseAutoFocus 把焦点还回去
  useEffect(() => {
    returnFocusRef.current =
      document.activeElement instanceof HTMLElement ? document.activeElement : null
  }, [])

  const submit = useCallback(async () => {
    if (submitting) return
    const name = value.trim()
    if (!name) {
      setError('文件夹名不能为空')
      return
    }
    setSubmitting(true)
    setError(null)
    let message: string | null
    try {
      message = await onSubmit(name)
    } catch (e) {
      message = e instanceof Error ? e.message : String(e)
    }
    // 成功时父级会卸载弹窗，这里不再 setState（避免卸载后更新）
    if (message) {
      setError(message)
      setSubmitting(false)
    }
  }, [onSubmit, submitting, value])

  const handleInputKeyDown = (event: ReactKeyboardEvent<HTMLInputElement>) => {
    if (event.key !== 'Enter' || event.nativeEvent.isComposing) return
    event.preventDefault()
    void submit()
  }

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        // 提交中不允许因 Esc / 点遮罩关闭；成功关闭由父级卸载
        if (!open && !submitting) onCancel()
      }}
    >
      <DialogContent
        className="kb-folder-dialog"
        overlayClassName="kb-folder-dialog-overlay"
        aria-describedby={undefined}
        onOpenAutoFocus={(event) => {
          event.preventDefault()
          const input = inputRef.current
          if (input) {
            input.focus()
            input.select()
          }
        }}
        onCloseAutoFocus={(event) => {
          event.preventDefault()
          returnFocusRef.current?.focus()
        }}
        onPointerDownOutside={(event) => {
          if (submitting) event.preventDefault()
        }}
      >
        <DialogTitle className="kb-folder-dialog-title">{title}</DialogTitle>
        <label className="kb-folder-dialog-label" htmlFor="kb-folder-name">
          文件夹名
        </label>
        <input
          id="kb-folder-name"
          ref={inputRef}
          className="kb-folder-dialog-input"
          value={value}
          disabled={submitting}
          placeholder="例如：会议纪要"
          onChange={(event) => {
            setValue(event.target.value)
            if (error) setError(null)
          }}
          onKeyDown={handleInputKeyDown}
        />
        <div className="kb-folder-dialog-error" role={error ? 'alert' : undefined}>
          {error ?? ''}
        </div>
        <div className="kb-folder-dialog-actions">
          <Button
            type="button"
            variant="ghost"
            size="xs"
            onClick={onCancel}
            disabled={submitting}
          >
            取消
          </Button>
          <Button
            type="button"
            variant="primary"
            size="xs"
            onClick={() => void submit()}
            disabled={submitting}
          >
            {submitting ? '处理中…' : submitLabel}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}