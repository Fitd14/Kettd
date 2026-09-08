import { useMemo, useState } from 'react'
import type { Bootstrap, Task } from '@/lib/api'
import { exportWeekly, trackEvent } from '@/lib/api'
import { TaskDetail } from '@/components/task-detail'
import { CompletionTimeline } from '@/components/completion-timeline'

interface Props {
  boot: Bootstrap
}

type Range = 'year' | 'd90' | 'month'

const RANGE_TABS: { key: Range; label: string; weeks: number }[] = [
  { key: 'year', label: '年', weeks: 52 },
  { key: 'd90', label: '近90天', weeks: 13 },
  { key: 'month', label: '本月', weeks: 5 },
]

/** 热力图档位（review-redesign-spec：0 / 1-2 / 3-5 / 6-9 / 10+） */
function level(n: number): number {
  if (n <= 0) return 0
  if (n <= 2) return 1
  if (n <= 5) return 2
  if (n <= 9) return 3
  return 4
}

/**
 * 回顾页（review-redesign-spec / timeline-component-spec）：
 * KPI 四项（本周完成 · 连续打卡 · 本年总计 · 无日期完成恒显）+ 周期 Tabs（年默认/近90天/本月）
 * + 热力图（列=周、行=周一..周日）+ 横向 CompletionTimeline + 点某天下钻。
 * 聚合口径：按 doneAt 日期聚合，排除回收站；legacy 不进格子、单列「无日期完成」——不伪造日期。
 */
export function ReviewView({ boot }: Props) {
  const [drill, setDrill] = useState<string | null>(null)
  const [range, setRange] = useState<Range>('year')
  const [detailId, setDetailId] = useState<string | null>(null)
  const [weeklyMsg, setWeeklyMsg] = useState<string | null>(null)
  const today = new Date()
  const todayIso = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, '0')}-${String(today.getDate()).padStart(2, '0')}`

  const done = useMemo(
    () => boot.tasks.filter((t) => t.done && !t.deletedAt && t.doneAt),
    [boot.tasks],
  )
  const legacyCount = useMemo(
    () => boot.tasks.filter((t) => t.done && !t.deletedAt && !t.doneAt).length,
    [boot.tasks],
  )

  const byDay = useMemo(() => {
    const map = new Map<string, Task[]>()
    for (const t of done) {
      const day = String(t.doneAt).slice(0, 10)
      const list = map.get(day) ?? []
      list.push(t)
      map.set(day, list)
    }
    return map
  }, [done])

  const weeks = RANGE_TABS.find((r) => r.key === range)?.weeks ?? 52

  // 热力图：以本周为末列，往回 weeks 列（列=周，行=周一..周日）
  const grid = useMemo(() => {
    const dow = (new Date(todayIso).getDay() + 6) % 7 // 周一=0
    const end = new Date(new Date(todayIso).getTime() + (6 - dow) * 86400000)
    const cols: { iso: string; n: number }[][] = []
    for (let w = weeks - 1; w >= 0; w -= 1) {
      const col: { iso: string; n: number }[] = []
      for (let d = 6; d >= 0; d -= 1) {
        const dt = new Date(end.getTime() - (w * 7 + d) * 86400000)
        const iso = `${dt.getFullYear()}-${String(dt.getMonth() + 1).padStart(2, '0')}-${String(dt.getDate()).padStart(2, '0')}`
        col.push({ iso, n: byDay.get(iso)?.length ?? 0 })
      }
      cols.push(col)
    }
    return cols
  }, [byDay, todayIso, weeks])

  const timeline = useMemo(
    () => [...done].sort((a, b) => String(b.doneAt).localeCompare(String(a.doneAt))),
    [done],
  )

  const kpis = [
    { label: '本周完成', value: done.filter((t) => String(t.doneAt).slice(0, 10) >= weekStart(todayIso)).length },
    { label: '连续打卡', value: streak(byDay, todayIso) },
    { label: '本年总计', value: done.filter((t) => String(t.doneAt).slice(0, 4) === todayIso.slice(0, 4)).length },
    { label: '无日期完成', value: legacyCount },
  ]

  const detailTask = boot.tasks.find((t) => t.id === detailId) ?? null

  return (
    <div className="view">
      <div>
        <div className="view-title">回顾</div>
        <div className="view-sub">
          看得见的节奏 · 全部数据来自本机 doneAt，回收站不计入 · legacy 不进格子
        </div>
      </div>

      <div className="row-flex" style={{ gap: 12, flexWrap: 'wrap' }}>
        {kpis.map((k) => (
          <div key={k.label} className="kpi">
            <div className="kpi-n">{k.value}</div>
            <div className="view-sub">{k.label}</div>
          </div>
        ))}
      </div>

      <section className="section" aria-label="完成热力图">
        <div className="section-head">
          <b>这一年</b>
          <div className="tabs" role="tablist" aria-label="统计周期">
            {RANGE_TABS.map((r) => (
              <button
                key={r.key}
                className={`tab${range === r.key ? ' on' : ''}`}
                role="tab"
                aria-selected={range === r.key}
                onClick={() => setRange(r.key)}
              >
                {r.label}
              </button>
            ))}
          </div>
        </div>
        <div className="tiny text-muted" style={{ marginBottom: 4 }}>浅 → 深 = 0 / 1-2 / 3-5 / 6-9 / 10+</div>
        <div className="heatmap" role="img" aria-label="按日完成密度热力图">
          {grid.map((col, i) => (
            <div className="heat-col" key={i}>
              {col.map((cell) => (
                <button
                  key={cell.iso}
                  className={`heat-cell lv-${level(cell.n)}`}
                  title={`${cell.iso} · 完成 ${cell.n}`}
                  aria-label={`${cell.iso} 完成 ${cell.n} 件`}
                  onClick={() => setDrill(cell.n > 0 ? cell.iso : null)}
                />
              ))}
            </div>
          ))}
        </div>
      </section>

      {drill && (
        <section className="section" aria-label={`${drill} 的完成`}>
          <div className="section-head">
            <b>{drill} 完成 {byDay.get(drill)?.length ?? 0} 件</b>
            <button className="btn ghost xs" onClick={() => setDrill(null)}>收起</button>
          </div>
          {(byDay.get(drill) ?? []).map((t) => (
            <div key={t.id} className="trow">
              <span className="grow truncate sm">{t.title}</span>
              <code className="tmeta mono">{String(t.doneAt).slice(11, 16)}</code>
              <span className={`cat-chip ${getCategoryColorCls(t.category)}`}>{t.category}</span>
            </div>
          ))}
        </section>
      )}

      <section className="section" aria-label="完成时间轴">
        <div className="section-head"><b>时间轴</b><span>最近在前 · 横向滚动</span></div>
        {timeline.length === 0 ? (
          <div className="empty">完成第一件事后，这里会按时间长出你的执行轨迹。</div>
        ) : (
          <CompletionTimeline items={timeline} onSelect={setDetailId} />
        )}
      </section>

      {/* 周汇总次级入口（review-redesign-spec §1 范围外保留项 / PRD 承诺 3；打开即记 weekly_open） */}
      <details
        className="section"
        aria-label="周汇总"
        onToggle={(e) => {
          if ((e.target as HTMLDetailsElement).open) {
            void trackEvent('weekly_open', { week: 'current' })
          }
        }}
      >
        <summary className="section-head"><b>周汇总草稿</b><span>展开生成 · 一键导出</span></summary>
        <p className="tiny text-muted">
          把本周完成/顺延整理成周报底稿（md），条目可勾选删改后交付——完成的不蒸发。
        </p>
        <button
          className="btn xs outline"
          onClick={() => {
            void (async () => {
              const r = await exportWeekly({ week: 'current', format: 'md' })
              setWeeklyMsg(r.err ?? `已导出：${r.data}`)
            })()
          }}
        >
          生成并导出本周汇总（md）
        </button>
        {weeklyMsg && <div className="tiny" style={{ marginTop: 6 }}>{weeklyMsg}</div>}
      </details>

      <TaskDetail task={detailTask} today={todayIso} onClose={() => setDetailId(null)} />
    </div>
  )
}

function getCategoryColorCls(category: string): string {
  // 与 api.ts CAT_CLS 同源（回顾页本地轻用，避免整包引入）
  return ({ '工作': 'cat-work', '学习': 'cat-study', '生活': 'cat-life' } as Record<string, string>)[category] ?? 'bg-muted text-muted'
}

function weekStart(iso: string): string {
  const d = new Date(iso)
  const dow = (d.getDay() + 6) % 7
  const m = new Date(d.getFullYear(), d.getMonth(), d.getDate() - dow)
  return `${m.getFullYear()}-${String(m.getMonth() + 1).padStart(2, '0')}-${String(m.getDate()).padStart(2, '0')}`
}

/** 连续天数：从今天（或昨天）往回数，每天都有完成才连续 */
function streak(byDay: Map<string, Task[]>, todayIso: string): number {
  let n = 0
  const cur = new Date(todayIso)
  if (!byDay.has(todayIso)) cur.setDate(cur.getDate() - 1)
  for (;;) {
    const iso = `${cur.getFullYear()}-${String(cur.getMonth() + 1).padStart(2, '0')}-${String(cur.getDate()).padStart(2, '0')}`
    if (!byDay.has(iso)) break
    n += 1
    cur.setDate(cur.getDate() - 1)
  }
  return n
}
