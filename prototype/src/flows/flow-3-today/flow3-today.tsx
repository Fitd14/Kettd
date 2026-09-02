/*
 * FLOW: Today Planning · 每日计划（story-4 ⭐ / PRD §6.3）
 * ENTRY: 主导航「今天」/ 引导跳转 / 通知深链 ?open=
 * SCREENS/态: ①常规（今日 · 已拖到今天 · 逾期折叠行 · 今日空态引导）②全部完成庆祝态
 * EXIT:
 *   ✅ Success: 全部勾完 → 完成态 + 明日预告
 *   ❌ Error: 勾选写盘失败 → 行内「没有保存 · 重试」（语义同 flow-6 底座）
 *   ↩ Abandon: 切走页面不丢状态（本地即时持久）
 */
import { useState, type ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent } from '@/components/ui/card'
import { Checkbox } from '@/components/ui/checkbox'
import { Progress } from '@/components/ui/progress'
import type { Task } from '../shared/types'
import { seedTasks, TODAY } from '../shared/mock-data'

const isCarry = (t: Task) => (t.carriedFromDays ?? 0) > 0
const isPlannedToday = (t: Task) => !t.done && (t.plannedDate === TODAY || (t.dueAt ?? '').startsWith(TODAY))
const isOverdue = (t: Task) => !t.done && !!t.dueAt && t.dueAt.slice(0, 10) < TODAY

const TaskRow = ({ t, onToggle, onPlan, failed, failRetry }: { t: Task; onToggle: (id: string) => void; onPlan?: (id: string) => void; failed?: boolean; failRetry?: () => void }) => (
  <Card className="border-0 shadow-none">
    <CardContent className="flex items-center gap-3 p-2.5">
      <Checkbox checked={t.done} onCheckedChange={(c) => c === true && onToggle(t.id)} aria-label={`完成 ${t.title}`} />
      <span className={`min-w-0 flex-1 truncate text-sm ${t.done ? 'text-muted-foreground line-through' : 'text-foreground'}`} title={t.title}>
        {t.title}
        {isCarry(t) && <Badge variant="outline" className="ml-2 text-[10px]">拖了 {t.carriedFromDays} 天</Badge>}
      </span>
      {onPlan && !isPlannedToday(t) && !t.done && (
        <Button size="sm" variant="ghost" className="h-6 px-2 text-[11px]" onClick={() => onPlan(t.id)}>加入今天</Button>
      )}
      {failed && <span className="flex shrink-0 items-center gap-1 text-[11px] text-destructive">没有保存<Button size="sm" variant="link" className="h-4 p-0 text-[11px]" onClick={failRetry}>重试</Button></span>}
      <span className="w-12 shrink-0 text-right font-mono text-[11px] text-muted-foreground">
        {t.dueAt ? (t.dueAt.slice(11) || '今天') : '—'}
      </span>
    </CardContent>
  </Card>
)

const SectionTitle = ({ children }: { children: ReactNode }) => (
  <div className="mb-1 mt-4 text-xs font-semibold uppercase tracking-wide text-muted-foreground">{children}</div>
)

export default function Flow3Today() {
  const [tasks, setTasks] = useState<Task[]>(() =>
    // 计划与逾期集合去重：逾期项不重复出现在今日
    seedTasks.map((t) => (isOverdue(t) ? { ...t, plannedDate: null } : t)))
  const [overdueOpen, setOverdueOpen] = useState(false)
  const [booting, setBooting] = useState(false)
  const [writable, setWritable] = useState(true)
  const [failedId, setFailedId] = useState<string | null>(null)

  const todayAll = tasks.filter(isPlannedToday)
  const carried = todayAll.filter(isCarry)
  const fresh = todayAll.filter((t) => !isCarry(t))
  const overdue = tasks.filter(isOverdue)
  const doneToday = tasks.filter((t) => t.done && (t.plannedDate === TODAY || (t.dueAt ?? '').startsWith(TODAY))).length
  const total = doneToday + todayAll.length

  const toggle = (id: string) => {
    if (!writable) { setWritable(true); setFailedId(id); return } // 演示：首点模拟写失败（勾选回滚不生效），重试即成功
    setTasks((p) => p.map((t) => (t.id === id ? { ...t, done: true, doneAt: `${TODAY}T14:0${1 + (id.charCodeAt(id.length - 1) % 8)}`, plannedDate: t.plannedDate ?? TODAY } : t)))
    setFailedId(null)
  }
  const planToday = (id: string) => setTasks((p) => p.map((t) => (t.id === id ? { ...t, plannedDate: TODAY } : t)))
  const reschedule = (id: string) => setTasks((p) => p.map((t) => (t.id === id ? { ...t, dueAt: undefined, carriedFromDays: 0 } : t)))

  const allDone = total > 0 && todayAll.length === 0

  return (
    <div className="mx-auto max-w-lg space-y-3 px-2 pt-4">
      <header className="flex items-baseline justify-between">
        <h1 className="text-lg font-semibold tracking-tight text-foreground">今天<span className="ml-2 text-xs font-normal text-muted-foreground">{doneToday}/{total} · 侧栏/悬浮面板/此处同源</span></h1>
      </header>
      <Progress value={total ? (doneToday / total) * 100 : 0} aria-label="今日进度" />
      <div className="flex justify-end gap-1">
        <Button size="sm" variant="ghost" className="h-6 px-2 text-[10px] text-muted-foreground" onClick={() => setWritable(false)}>演示：写失败后点勾</Button>
        <Button size="sm" variant="ghost" className="h-6 px-2 text-[10px] text-muted-foreground" onClick={() => setBooting((v) => !v)}>{booting ? '结束慢盘模拟' : '模拟慢盘启动'}</Button>
      </div>

      {booting && (
        // STATE: loading-initial（本地 <1s 静默；此按钮演示 >3s 骨架档 + 取消出口）
        <div className="space-y-2" role="status">
          {[70, 85, 60].map((w, i) => <div key={i} className="h-10 animate-pulse rounded-lg bg-muted" style={{ width: `${w}%` }} />)}
          <div className="text-center text-[11px] text-muted-foreground">正在读取本机数据…<Button size="sm" variant="link" className="h-4 p-0 align-baseline text-[11px]" onClick={() => setBooting(false)}>取消等待</Button></div>
        </div>
      )}

      {!booting && allDone && (
        // STATE: all-done 庆祝态（轻量、1s 内不弹跳）
        <Card className="border-input bg-accent/50"><CardContent className="p-6 text-center">
          <div className="text-2xl" aria-hidden>✓</div>
          <div className="mt-1 font-medium text-foreground">今天清空了。</div>
          <p className="mt-1 text-xs text-muted-foreground">明早 1 条到期 —— 睡前想记的，Alt+Shift+A。</p>
        </CardContent></Card>
      )}

      {!booting && !allDone && (<>
        {fresh.length === 0 && carried.length === 0 ? (
          // STATE: empty —— 引导下一步而非空盒
          <Card><CardContent className="p-4 text-sm text-muted-foreground">
            今天还是空的。从「即将到来」挑，或按 <kbd className="rounded border bg-muted px-1 font-mono text-xs">Alt+Shift+A</kbd> 记一条新的事。
          </CardContent></Card>
        ) : (
          <>
            {fresh.length > 0 && <><SectionTitle>今天要做的 · {fresh.length}</SectionTitle>{fresh.map((t) => <TaskRow key={t.id} t={t} onToggle={toggle} failed={failedId === t.id} failRetry={() => toggle(t.id)} />)}</>}
            {carried.length > 0 && <><SectionTitle>已拖到今天 · {carried.length}</SectionTitle>{carried.map((t) => <TaskRow key={t.id} t={t} onToggle={toggle} failed={failedId === t.id} failRetry={() => toggle(t.id)} />)}</>}
          </>
        )}

        {overdue.length > 0 && (
          <section>
            <Button variant="ghost" className="mt-4 h-8 w-full justify-start px-2 text-xs text-destructive" onClick={() => setOverdueOpen((v) => !v)} aria-expanded={overdueOpen}>
              {overdueOpen ? '▾' : '▸'} 已逾期 {overdue.length} 项 —— {overdueOpen ? '收起' : '逐条安排'}
            </Button>
            {overdueOpen && overdue.map((t) => (
              <Card key={t.id}><CardContent className="flex items-center gap-2 p-2.5 text-sm">
                <span className="flex-1 truncate">{t.title}
                  <span className="ml-2 font-mono text-[10px] text-muted-foreground">{t.dueAt?.slice(0, 10)}</span></span>
                <Button size="sm" variant="outline" className="h-6 text-[11px]" onClick={() => planToday(t.id)}>移到今天</Button>
                <Button size="sm" variant="ghost" className="h-6 text-[11px]" onClick={() => reschedule(t.id)}>清截止</Button>
                {/* 丢弃 = 软删，进 flow-6 的撤销 toast 语义 */}
                <Button size="sm" variant="ghost" className="h-6 text-[11px] text-destructive" onClick={() => setTasks((p) => p.filter((x) => x.id !== t.id))}>丢弃</Button>
              </CardContent></Card>
            ))}
          </section>
        )}

        <SectionTitle>可拉进今天（收件箱 / 即将到来）</SectionTitle>
        {tasks.filter((t) => !t.done && !isPlannedToday(t) && !isOverdue(t)).slice(0, 4).map((t) => (
          <TaskRow key={t.id} t={t} onToggle={toggle} onPlan={planToday} />
        ))}
      </>)}

      <p className="pt-2 text-center text-[11px] text-muted-foreground">t 键 = 加入今天 · 空格 = 勾完（PRD 6.3 键盘通道）</p>
    </div>
  )
}
