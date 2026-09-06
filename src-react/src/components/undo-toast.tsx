import { useCallback, useEffect, useRef, useState } from 'react'

/**
 * 软删 10s 撤销链路的 UI 侧（PRD 6.6 / M2 底座）。
 * 用法：const toast = useUndoToast(); 删除时 toast.arm(id, title, () => api.undoDelete());
 * 组件树里渲染 <UndoToast state={toast.state} onUndo={toast.undo} onDismiss={toast.dismiss} />。
 * 语义与 vanilla 对齐：10s 窗口；撤销失败的 err 原样展示；超时自动消失（后端软删仍在回收站兜底）。
 */
export interface UndoToastState {
  visible: boolean
  title: string
  error: string | null
  secondsLeft: number
}

const WINDOW_MS = 10_000

export function useUndoToast() {
  const [state, setState] = useState<UndoToastState>({
    visible: false,
    title: '',
    error: null,
    secondsLeft: WINDOW_MS / 1000,
  })
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const undoRef = useRef<(() => Promise<{ err: string | null }>) | null>(null)

  const stop = useCallback(() => {
    if (timerRef.current) {
      clearInterval(timerRef.current)
      timerRef.current = null
    }
  }, [])

  const dismiss = useCallback(() => {
    stop()
    undoRef.current = null
    setState((s) => ({ ...s, visible: false, error: null }))
  }, [stop])

  /** 拉起撤销条：undo 动作必须是「撤销最近一次删除」的调用（api.undoDelete） */
  const arm = useCallback((title: string, undo: () => Promise<{ err: string | null }>) => {
    stop()
    undoRef.current = undo
    setState({ visible: true, title, error: null, secondsLeft: WINDOW_MS / 1000 })
    timerRef.current = setInterval(() => {
      setState((s) => {
        if (s.secondsLeft <= 1) {
          stop()
          undoRef.current = null
          return { ...s, visible: false, error: null, secondsLeft: 0 }
        }
        return { ...s, secondsLeft: s.secondsLeft - 1 }
      })
    }, 1000)
  }, [stop])

  const undo = useCallback(async () => {
    const action = undoRef.current
    if (!action) return
    const r = await action()
    if (r.err) {
      // 撤销失败：不关条，把后端短句展示出来（写失败时用户可重试）
      setState((s) => ({ ...s, error: r.err }))
      return
    }
    dismiss()
  }, [dismiss])

  useEffect(() => stop, [stop])

  return { state, arm, undo, dismiss }
}

export function UndoToast({
  state,
  onUndo,
  onDismiss,
}: {
  state: UndoToastState
  onUndo: () => void
  onDismiss: () => void
}) {
  if (!state.visible) return null
  return (
    <div className="undo-toast" role="status" aria-live="polite">
      <span className="undo-title">
        已删除「{state.title}」
      </span>
      {state.error && <span className="undo-err">{state.error}</span>}
      <button className="undo-btn" onClick={onUndo}>
        撤销（{state.secondsLeft}s）
      </button>
      <button className="undo-x" aria-label="关闭" onClick={onDismiss}>✕</button>
    </div>
  )
}
