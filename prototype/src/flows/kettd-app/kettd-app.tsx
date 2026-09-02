/*
 * APP SHELL · Kettd v2（PRD §8 Wave-1 范围的全部入口，路由对齐 sitemap.routing_table）
 * 布局：左 sidebar 5 项主导航（sitemap.primary_navigation）+ 右侧「流程演示」直达组
 * 主题：shadcn class 切换（明暗双态，PRD 6.8；生产环境经 token css 同源移植 Tauri 侧）
 */
import { useEffect, useState } from 'react'
import { Routes, Route, Navigate, NavLink, Link } from 'react-router-dom'
import { Inbox, CalendarCheck, CalendarClock, BookOpen, Settings } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import Flow1QuickCapture from '../flow-1-capture/flow1-quick-capture'
import Flow2FirstRun from '../flow-2-first-run/flow2-onboarding'
import Flow3Today from '../flow-3-today/flow3-today'
import Flow4Reminders from '../flow-4-reminders/flow4-reminders'
import Flow5WeeklyReview from '../flow-5-review/flow5-weekly-review'
import Flow6Recovery from '../flow-6-recovery/flow6-recovery'
import FlowBootMigration from '../flow-boot/flow-boot-migration'
import FlowSearchOverlay from '../flow-search/flow-search'

const NAV = [
  { to: '/today', label: '今天', icon: CalendarCheck },
  { to: '/inbox', label: '收件箱', icon: Inbox },
  { to: '/planned', label: '计划', icon: CalendarClock },
  { to: '/review', label: '回顾', icon: BookOpen },
  { to: '/settings', label: '设置', icon: Settings },
]

const DEMO_LINKS = [
  { to: '/flows/capture', label: 'F1 3 秒捕获 ⭐' },
  { to: '/flows/onboarding', label: 'F2 首启三课 ⭐' },
  { to: '/flows/reminders', label: 'F4 到点真提醒 ⭐' },
  { to: '/flows/recovery', label: 'F6 撤销与恢复' },
  { to: '/search', label: 'F7 全局搜索' },
  { to: '/boot/migration', label: 'F0 迁移兜底' },
]

function SettingsScreen({ isDark, setDark }: { isDark: boolean; setDark: (v: boolean) => void }) {
  return (
    <div className="mx-auto max-w-md space-y-4 pt-6 text-sm">
      <h1 className="text-lg font-semibold tracking-tight">设置</h1>
      <div className="rounded-xl border bg-card p-4"><div className="flex items-center justify-between">
        <span>主题 · {isDark ? '暗色（夜间学习）' : '纸白'}</span>
        <Button size="sm" variant="outline" onClick={() => setDark(!isDark)}>{isDark ? '‹ 切回纸白' : '‹ 切到暗色'}</Button></div>
      </div>
      <div className="rounded-xl border bg-card p-4 text-xs text-muted-foreground">数据 · 备份 5 档自动轮转 · <Link to="/flows/recovery" className="text-primary underline">恢复与演示 →</Link></div>
      <div className="rounded-xl border bg-card p-4 text-xs text-muted-foreground">快捷键 · Alt+Shift+A 快录（被占时点「点这里代替按键」） · t 加入今天 · 空格 勾完</div>
    </div>
  )
}

/* 深链 ?open=:id —— 通知/搜索直达的占位实现（读取参数并高亮对应项由 Flow3 内联完成） */
const InboxRoute = () => <Flow3Today /> // 收件箱拉取区已并入今天屏底部（PRD 6.3“可拉进今天”）
const PlannedRoute = () => <Flow4Reminders /> // 面板形态 + 提醒管理上下文（孤岛页归位后的宿主）

export function KettdApp() {
  const [isDark, setIsDark] = useState(false)
  useEffect(() => {
    document.documentElement.classList.toggle('dark', isDark)
  }, [isDark])

  return (
    <div className="flex min-h-screen bg-background">
      <aside className="flex w-56 shrink-0 flex-col gap-1 border-r bg-card p-3">
        <div className="mb-2 flex items-center gap-2 px-2 pt-1">
          <span className="inline-block h-4 w-4 rounded-[5px] bg-primary" aria-hidden />
          <span className="text-sm font-semibold tracking-tight">Kettd <span className="font-normal text-muted-foreground">v2</span></span>
        </div>
        {NAV.map(({ to, label, icon: Icon }) => (
          <NavLink key={to} to={to} className={({ isActive }) =>
            `flex items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-sm transition-colors hover:bg-muted ${isActive ? 'bg-secondary font-medium text-foreground' : 'text-muted-foreground'}`}>
            <Icon className="h-4 w-4" aria-hidden />{label}
          </NavLink>
        ))}
        <Separator className="my-3" />
        <div className="px-2 text-[11px] font-semibold uppercase tracking-wide text-muted-foreground">流程演示（Phase B 产物）</div>
        {DEMO_LINKS.map(({ to, label }) => (
          <NavLink key={to} to={to} className={({ isActive }) =>
            `rounded-lg px-2.5 py-1 text-xs ${isActive ? 'bg-accent font-medium text-accent-foreground' : 'text-muted-foreground hover:bg-muted'}`}>
            {label}</NavLink>
        ))}
        <div className="mt-auto flex flex-wrap gap-1.5 px-1">
          {[['F3', '/today'], ['F5', '/review']].map(([t, to]) => (
            <Link key={t} to={to}><Button size="sm" variant="outline" className="h-6 px-2 text-[10px]" title={to}>{t}</Button></Link> // 导航直达=入口本体，F3/F5 无重复演示页
          ))}
        </div>
      </aside>
      <main className="min-w-0 flex-1 overflow-y-auto">
        <Routes>
          <Route path="/" element={<Navigate to="/today" replace />} />
          <Route path="/today" element={<Flow3Today />} />
          <Route path="/inbox" element={<InboxRoute />} />
          <Route path="/planned" element={<PlannedRoute />} />
          <Route path="/review" element={<Flow5WeeklyReview />} />
          <Route path="/settings" element={<SettingsScreen isDark={isDark} setDark={setIsDark} />} />
          {/* 深链协议占位：/:view?open=id —— 路由可解析即可，选中高亮在 flow 内 */}
          <Route path="/flows/capture" element={<Flow1QuickCapture />} />
          <Route path="/flows/onboarding" element={<Flow2FirstRun />} />
          <Route path="/flows/reminders" element={<PlannedRoute />} />
          <Route path="/flows/recovery" element={<Flow6Recovery />} />
          <Route path="/search" element={<FlowSearchOverlay />} />
          <Route path="/boot/migration" element={<FlowBootMigration />} />
          <Route path="*" element={<Navigate to="/today" replace />} />
        </Routes>
      </main>
    </div>
  )
}
