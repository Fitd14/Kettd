/*
 * FLOW: Quick Capture · 3 秒捕获（story-1 ⭐ / PRD §6.1）
 * ENTRY: 全局热键 Alt+Shift+A（生产由 Tauri global-shortcut 触发；原型内以悬浮条模拟）
 * SCREENS: 3 —— ①CaptureBar ②落库回执 Landed ③InboxLanding（收件箱高亮）
 * EXIT:
 *   ✅ Success: chips+「已记下」，任务进入收件箱，dueAt 本地语义无偏移
 *   ❌ Error: 保存失败 → 红色内联提示 + 重试，输入不丢
 *   ↩ Abandon: Esc / 点外部 → 输入即丢（PRD 语义：回车才落库，未回车不落草稿）
 */
import { useMemo, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent } from '@/components/ui/card'
import { Separator } from '@/components/ui/separator'
import { parseCapture, seedTasks, TODAY } from '../shared/mock-data'

export default function Flow1QuickCapture() {
  const [step, setStep] = useState(1)
  const [text, setText] = useState('')
  const [error, setError] = useState(false)
  // STATE: default(输入条) | typing | error:save-failed
  const parsed = useMemo(() => parseCapture(text, TODAY), [text])

  const commit = () => {
    if (!text.trim()) return
    if (text.trim() === 'fail') { setError(true); return } // 演示错误态
    setError(false)
    setStep(2)
  }

  return (
    <div className="mx-auto max-w-md space-y-4 pt-6">
      {step === 1 && (
        // SCREEN 1 of 3 · CaptureBar
        <Card className="border-2 shadow-lg">
          <CardContent className="space-y-3 p-4">
            <Input
              autoFocus
              value={text}
              onChange={(e) => setText(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') commit()
                if (e.key === 'Escape') { setText(''); setError(false) } // ↩ Abandon
              }}
              maxLength={200} placeholder="明天下午3点找导师 #学习 !高 …（输‘fail’演示失败态）"
            />
            {parsed.chips.length > 0 && !error && (
              <div className="flex flex-wrap gap-2">
                {parsed.chips.map((c) => (
                  <Button key={c.raw} size="sm" variant="secondary" className="h-6 gap-1 px-2 text-xs">
                    {c.value}
                    <span className="text-muted-foreground">×</span> {/* 点击可改：重开补正（规格见 PRD 6.1 chip 修正） */}
                  </Button>
                ))}
                {parsed.autoRemind && <Badge variant="outline" className="text-[10px]">将随截止自动提醒</Badge>}
              </div>
            )}
            {error && (
              // STATE: error:save-failed —— 输入不丢，重试通道
              <div className="flex items-center gap-2 rounded-md bg-destructive/10 p-2 text-sm text-destructive">
                没有保存 · 磁盘写入失败
                <Button size="sm" variant="destructive" onClick={() => { setError(false); setStep(2) }}>重试</Button>
              </div>
            )}
            <div className="text-xs text-muted-foreground">{text.trim() ? `标题「${parsed.title}」 · 回车落库 · Esc 放弃` : '按 Alt+Shift+A 从任意应用唤起（此处为模拟屏）'}</div>
          </CardContent>
        </Card>
      )}
      {/* → 用户按 Enter 且保存成功 → SCREEN 2 */}
      {step === 2 && (
        // SCREEN 2 of 3 · Landed（强反馈但不抢回焦点）
        <Card className="border-input bg-accent/50">
          <CardContent className="flex items-center justify-between p-4">
            <div>
              <div className="text-sm font-medium">已记下 ·「{parsed.title}」</div>
              <div className="mt-1 flex gap-2">
                {parsed.chips.map((c) => <Badge key={c.raw} variant="outline" className="text-[10px]">{c.value}</Badge>)}
                {parsed.fellBack && <Badge variant="outline" className="text-[10px]">纯文本 · 收件箱待整理</Badge>}
              </div>
            </div>
            <Button size="sm" onClick={() => setStep(3)}>{'加入今天'}</Button>
          </CardContent>
        </Card>
      )}
      {/* → 用户点「加入今天」→ SCREEN 3；5s 无操作自动收起= Abandon 完成态 */}
      {step === 3 && (
        // SCREEN 3 of 3 · InboxLanding —— 捕获结果在收件箱置顶可见
        <>
          <Separator />
          <div className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">收件箱 · 刚刚</div>
          {[{ title: parsed.title || '示例条目', dueAt: parsed.dueAt ?? null, fresh: true }, ...seedTasks.slice(0, 2).map((t) => ({ title: t.title, dueAt: t.dueAt ?? null, fresh: false }))].map((t, i) => (
            <Card key={i} className={t.fresh ? 'ring-1 ring-accent' : ''}>
              <CardContent className="flex items-baseline justify-between p-3 text-sm">
                <span className="min-w-0 flex-1 truncate" title={t.title}>{t.title}</span>
                <span className="font-mono text-[11px] text-muted-foreground">{t.dueAt ?? '无截止'}</span>
              </CardContent>
            </Card>
          ))}
        </>
      )}
    </div>
  )
}
