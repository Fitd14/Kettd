/*
 * FLOW: Reminder Arrival · 到点真提醒（story-5 ⭐ / PRD §6.4）
 * ENTRY: 系统事件（remindAt 到点）/ 面板形态切换 / 权限缺失首触发
 * SCREENS: 3 —— ①通知卡（含 snooze）②悬浮面板三形态切换 ③权限缺失引导；末附加：未运行补发态
 * EXIT:
 *   ✅ Success: 点通知深链直达任务（此处以跳转占位），或 snooze 后再达
 *   ❌ Error: 系统通知权限未开 → 引导卡（一键 ms-settings）
 *   ↩ Abandon: 通知被系统丢弃 → 下次启动经「补发通道」回到迷你条
 */
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Separator } from '@/components/ui/separator'
import type { FloatForm } from '../shared/types'

const FORM_LABEL: Record<FloatForm, string> = { alwaysOnTop: '置于顶层', embedded: '嵌入桌面', mini: '迷你条' }

export default function Flow4Reminders() {
  const [fired, setFired] = useState(true)
  const [form, setForm] = useState<FloatForm>('alwaysOnTop')
  const [permDemo, setPermDemo] = useState(false)
  const [snoozed, setSnoozed] = useState<string | null>(null)
  const [remaining, setRemaining] = useState(3)
  const [remList, setRemList] = useState([
    { id: 'r-01', title: '季度复盘 · 16:30 随截止', at: '2026-09-02 16:30' },
    { id: 'r-02', title: '高数课后题 · 20:00 随截止', at: '2026-09-02 20:00' },
  ])
  const [rTitle, setRTitle] = useState('')
  const [rAt, setRAt] = useState('')
  const invalid = rAt !== '' && rAt < '2026-09-02 16:20'
  const addReminder = () => {
    if (!rTitle.trim() || !rAt || invalid) return
    setRemList((p) => [...p, { id: Math.random().toString(36).slice(2, 6), title: rTitle, at: rAt }])
    setRTitle(''); setRAt('')
  }

  return (
    <div className="mx-auto max-w-md space-y-4 pt-4">
      <div className="flex items-center justify-between text-xs text-muted-foreground">
        <span>提醒引擎 · 状态模拟</span>
        <div className="flex gap-2">
          <Button size="sm" variant="outline" className="h-6 text-[11px]" onClick={() => { setFired(true); setSnoozed(null); setPermDemo(false) }}>再触发一次</Button>
          <Button size="sm" variant="ghost" className="h-6 text-[11px]" onClick={() => { setFired(false); setPermDemo(true) }}>演示权限缺失态</Button>
          <Button size="sm" variant="ghost" className="h-6 text-[11px]" onClick={() => { setFired(false); setPermDemo(false) }}>演示未运行补发</Button>
        </div>
      </div>

      {/* SCREEN 1 · 通知 + snooze */}
      {fired && !permDemo && (
        <Card className="border-input shadow-lg">
          <CardContent className="space-y-2 p-4">
            <div className="flex items-center gap-2 text-[11px] text-muted-foreground"><span aria-hidden>🔔</span> 待办提醒 · 现在
              <Badge variant="outline" className="ml-auto text-[10px]">r-01</Badge></div>
            <div className="text-sm font-medium text-foreground">该写季度复盘了 —— 17:00 截止（还剩 30 分钟）</div>
            <div className="flex gap-2 pt-1">
              {/* 点击主体 = 深链直达（?open=t-01） */}
              {snoozed === null ? (<>
                <Button size="sm" variant="outline" className="h-7 text-xs" onClick={() => setSnoozed('16:40 · 将再次触达（≤3 次/小时防骚扰）')}>{'稍后 10 分钟'}</Button>
                <Button size="sm" variant="outline" className="h-7 text-xs" onClick={() => setSnoozed('今晚 20:30 · 已并入晚间档')}>{'今天晚些'}</Button>
              </>) : <span className="text-xs text-muted-foreground">已延后 → {snoozed}</span>}
              <Button size="sm" className="h-7 text-xs" onClick={() => setFired(false)}>{'去看这条'}</Button>
            </div>
          </CardContent>
        </Card>
      )}
      {/* 边缘 · 未运行补发态 */}
      {!fired && !permDemo && (
        <Card className="border-dashed"><CardContent className="p-3 text-xs text-muted-foreground">
          错过 1 条提醒（16:30）· 「去看这条」仍可进任务。<span className="text-foreground">未运行且无计划任务时，启动即补达</span>（必达承诺边界，PRD 6.4）。
        </CardContent></Card>
      )}
      {/* SCREEN 3 · 权限缺失引导 */}
      {permDemo && (
        <Card className="border-amber-500/50"><CardContent className="space-y-2 p-4 text-sm">
          <div className="font-medium text-amber-700">系统没让 Kettd 弹通知</div>
          <p className="text-xs text-muted-foreground">点下面跳到 Windows 设置打开开关；在此之前提醒照常进面板迷你条，不失联。</p>
          <Button size="sm" variant="outline">打开通知设置（ms-settings:notifications）</Button>
        </CardContent></Card>
      )}

      <div className="flex justify-end"><Button size="sm" variant="ghost" className="h-6 px-2 text-[10px] text-muted-foreground" onClick={() => setRemaining(remaining === 3 ? 128 : remaining === 128 ? 0 : 3)}>演示剩 {remaining === 3 ? '128 件（百级）' : remaining === 128 ? '0（清零）' : '3 件'}</Button></div>

      <Card>
        <CardHeader className="pb-2"><CardTitle className="text-sm">提醒管理 · {remList.length} 条</CardTitle></CardHeader>
        <CardContent className="space-y-2">
          {remList.length === 0 ? (
            <div className="rounded-lg border border-dashed border-input p-3 text-center text-xs text-muted-foreground">还没有提醒 —— 以后在任务上点 ⏰ 就地加，这里看全量。<Button size="sm" variant="link" className="h-4 p-0 align-baseline text-xs">教我这个手势</Button></div>
          ) : remList.map((r) => (
            <div key={r.id} className="flex items-center gap-2 text-sm">
              <span className="min-w-0 flex-1 truncate" title={r.title}>{r.title}</span>
              <code className="font-mono text-[10px] text-muted-foreground">{r.at}</code>
              <Button size="sm" variant="ghost" className="h-6 px-1 text-[11px] text-destructive" onClick={() => setRemList((p) => p.filter((x) => x.id !== r.id))}>删</Button>
            </div>
          ))}
          <div className="flex flex-wrap gap-2 pt-1">
            <input value={rTitle} onChange={(e) => setRTitle(e.target.value)} placeholder="标题，如：取快递" className="h-8 min-w-0 flex-1 rounded-md border border-input bg-background px-2 text-xs" />
            <input value={rAt} onChange={(e) => setRAt(e.target.value)} placeholder="2026-09-02 18:00" className="h-8 w-44 rounded-md border border-input bg-background px-2 font-mono text-xs" />
            <Button size="sm" variant="secondary" className="h-8 text-xs" onClick={addReminder}>添加</Button>
          </div>
          {invalid && (
            <div className="flex items-center gap-2 rounded-md bg-amber-500/10 px-2 py-1.5 text-[11px] text-amber-700">这已是过去时间
              <Button size="sm" variant="outline" className="h-6 text-[11px]" onClick={() => { setRemList((p) => [...p, { id: 'auto', title: `${rTitle || '提醒'} · 刚刚+5min`, at: '2026-09-02 16:25' }]); setRTitle(''); setRAt('') }}>按最近可及记</Button>
              <Button size="sm" variant="ghost" className="h-6 text-[11px]" onClick={() => { setRemList((p) => [...p, { id: 'raw', title: rTitle, at: rAt + '（不响）' }]); setRTitle(''); setRAt('') }}>仍照原样，不响</Button>
            </div>
          )}
        </CardContent>
      </Card>

      <Separator />

      {/* SCREEN 2 · 面板三形态（常驻不挡路承诺的具体面） */}
      <Card>
        <CardHeader className="pb-2"><CardTitle className="text-sm">悬浮面板 · 三形态（{FORM_LABEL[form]}）</CardTitle></CardHeader>
        <CardContent className="space-y-3">
          <Tabs value={form} onValueChange={(v) => setForm(v as FloatForm)}>
            <TabsList className="w-full"><TabsTrigger value="alwaysOnTop">置顶</TabsTrigger><TabsTrigger value="embedded">嵌桌面</TabsTrigger><TabsTrigger value="mini">迷你</TabsTrigger></TabsList>
          </Tabs>
          {form === 'mini' ? (
            // 迷你条：高 ≤56px 仅两行——常驻但退化成桌面边缘一撇
            <div className="flex h-12 w-52 items-center justify-between rounded-lg border border-input bg-card px-3 shadow-sm">
              <div className="text-[11px] leading-tight text-muted-foreground">今天剩 <b className="text-sm text-foreground">{remaining > 99 ? '99+' : remaining === 0 ? '清零 ✓' : remaining}</b>{remaining === 0 ? '' : ' 件'}</div>
              <div className="text-[11px] text-muted-foreground">下一条 <span className="font-mono text-foreground">16:30</span></div>
            </div>
          ) : (
            <Card className={form === 'embedded' ? 'bg-muted/50 shadow-none' : ''}>
              <CardContent className="p-3 text-xs leading-5">
                <div className="mb-1 font-medium text-foreground">今天 · 3 件 <span className="font-normal text-muted-foreground">（{form === 'embedded' ? '不抢焦点 · 点入即编辑' : '盖在其他窗口上'}）</span></div>
                取快递（丰巢）<span className="text-muted-foreground"> · 已拖到今天</span>
                <div>季度复盘 <span className="ml-1 text-muted-foreground">16:30</span></div>
                <div>高数课后题 <span className="ml-1 text-muted-foreground">20:00</span></div>
                <div className="mt-2 flex items-center gap-2 text-muted-foreground">· 30s 事件同步（无轮询）· 1 秒不活跃自动淡出</div>
              </CardContent>
            </Card>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
