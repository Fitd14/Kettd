import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  BookOpen,
  CalendarDays,
  Inbox,
  PanelLeftClose,
  PanelLeftOpen,
  RotateCcw,
  Settings,
  Sun,
  type LucideIcon,
} from 'lucide-react'
import {
  applyTheme,
  getBootstrap,
  onEvent,
  takePendingRoute,
  type Bootstrap,
} from '@/lib/api'
import { todaySections } from './kernel/selectors.js'
import { localToday } from './kernel/time.js'
import { useUndoToast, UndoToast } from '@/components/undo-toast'
import { TitleBar } from '@/components/title-bar'
import { TodayView } from '@/views/TodayView'
import { InboxView } from '@/views/InboxView'
import { PlannedView } from '@/views/PlannedView'
import { ReviewView } from '@/views/ReviewView'
import { KbView } from '@/views/KbView'
import { SettingsView } from '@/views/SettingsView'
import './styles/app.css'

const ROUTES = [
  { hash: '#/today', label: '今天', icon: Sun },
  { hash: '#/inbox', label: '收件箱', icon: Inbox },
  { hash: '#/planned', label: '计划', icon: CalendarDays },
  { hash: '#/review', label: '回顾', icon: RotateCcw },
  { hash: '#/kb', label: '知识库', icon: BookOpen },
  { hash: '#/settings', label: '设置', icon: Settings },
] as const satisfies ReadonlyArray<{ hash: string; label: string; icon: LucideIcon }>

type Route = (typeof ROUTES)[number]['hash']

function routeFromHash(): Route {
  // Deep Link 带查询串（#/kb?id=x）——按 ? 切掉再匹配，否则整串对不上回落今天页
  const h = (window.location.hash.split('?')[0] || window.location.hash) as Route
  return ROUTES.some((r) => r.hash === h) ? h : '#/today'
}

/**
 * 应用外壳（M3）：侧栏五视图 + 单一真相源（后端 Store）。
 * 跨窗一致性沿用契约：store-changed 广播 → 重拉（api.ts 的补发队列保证启动竞态不丢事件）。
 */
export default function App() {
  const [route, setRoute] = useState<Route>(routeFromHash)
  const [boot, setBoot] = useState<Bootstrap | null>(null)
  const [error, setError] = useState<string | null>(null)
  // 主侧栏折叠（#13）：localStorage 持久化，重启保持
  const [sideCollapsed, setSideCollapsed] = useState(
    () => localStorage.getItem('shell.sideCollapsed') === '1',
  )
  const undo = useUndoToast()

  const toggleSide = useCallback(() => {
    setSideCollapsed((prev) => {
      localStorage.setItem('shell.sideCollapsed', prev ? '0' : '1')
      return !prev
    })
  }, [])

  const refresh = useCallback(async () => {
    const r = await getBootstrap()
    if (r.err || !r.data) {
      setError(r.err ?? '启动加载失败')
      return
    }
    setError(null)
    setBoot(r.data)
    applyTheme(r.data.settings.theme)
  }, [])

  useEffect(() => {
    void refresh()
    // E14：store-changed 在订阅前到达会先进补发队列，挂上监听即回放
    const off = onEvent('store-changed', () => { void refresh() })
    // 跨窗导航兜底：主窗 focus 时消费 localStorage 里的待跳转路由
    const pending = takePendingRoute()
    if (pending) {
      const hash = `#${pending.split('?')[0]}`
      if (ROUTES.some((r) => r.hash === hash)) {
        window.location.hash = hash
        setRoute(hash as Route)
      }
    }
    const onHash = () => setRoute(routeFromHash())
    window.addEventListener('hashchange', onHash)
    return () => { off(); window.removeEventListener('hashchange', onHash) }
  }, [refresh])

  const today = localToday()
  const todayCount = useMemo(() => {
    if (!boot) return 0
    const s = todaySections(boot.tasks, today)
    return s.due.length + s.carried.length
  }, [boot, today])
  const inboxCount = useMemo(() => {
    if (!boot) return 0
    return boot.tasks.filter(
      (t) => !t.done && !t.deletedAt && !t.plannedDate && !t.dueAt,
    ).length
  }, [boot])

  const counts: Partial<Record<Route, number>> = {
    '#/today': todayCount,
    '#/inbox': inboxCount,
  }

  return (
    <div className="app-frame">
      <TitleBar />
      <div className="shell">
        <nav
          className={`shell-side${sideCollapsed ? ' is-collapsed' : ''}`}
          aria-label="视图"
        >
          <div className="brand" title="Kettd">{sideCollapsed ? 'K' : 'Kettd'}</div>
          {ROUTES.map((r) => {
            const Icon = r.icon
            return (
              <button
                key={r.hash}
                className={`side-item${route === r.hash ? ' on' : ''}`}
                title={r.label}
                onClick={() => { window.location.hash = r.hash; setRoute(r.hash) }}
              >
                <Icon className="side-icon" size={16} aria-hidden="true" />
                <span className="label">{r.label}</span>
                {counts[r.hash] ? <span className="count">{counts[r.hash]}</span> : null}
              </button>
            )
          })}
          <button
            className="side-item side-toggle"
            onClick={toggleSide}
            title={sideCollapsed ? '展开侧栏' : '收起侧栏'}
            aria-label={sideCollapsed ? '展开侧栏' : '收起侧栏'}
            aria-expanded={!sideCollapsed}
          >
            {sideCollapsed
              ? <PanelLeftOpen className="side-icon" size={16} aria-hidden="true" />
              : <PanelLeftClose className="side-icon" size={16} aria-hidden="true" />}
            <span className="label">{sideCollapsed ? '展开' : '收起'}</span>
          </button>
        </nav>
        <main className="shell-main">
          {error && (
            <div className="view">
              <div className="inline-err" role="alert">{error}</div>
            </div>
          )}
          {!error && !boot && (
            <div className="view">
              <div className="empty">加载中…</div>
            </div>
          )}
          {boot && route === '#/today' && (
            <TodayView boot={boot} refresh={refresh} undo={undo} />
          )}
          {boot && route === '#/inbox' && (
            <InboxView boot={boot} refresh={refresh} undo={undo} />
          )}
          {boot && route === '#/planned' && <PlannedView boot={boot} refresh={refresh} />}
          {boot && route === '#/review' && <ReviewView boot={boot} />}
          {boot && route === '#/kb' && <KbView boot={boot} />}
          {boot && route === '#/settings' && <SettingsView boot={boot} refresh={refresh} />}
        </main>
        <UndoToast state={undo.state} onUndo={() => { void undo.undo() }} onDismiss={undo.dismiss} />
      </div>
    </div>
  )
}
