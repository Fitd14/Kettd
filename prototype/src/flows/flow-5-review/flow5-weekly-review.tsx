/*
 * FLOW: Weekly Review · 周汇总草稿（story-6 ⭐差异化主轴 / PRD §6.5）
 * ENTRY: /review 导航 · 周五 07:30 自动草稿徽标点击 · 日志簿翻页
 * SCREENS: 3 —— ①日志簿（按日分组）②本周草稿（三桶统计+条目勾选删改）③导出结果态
 * EXIT:
 *   ✅ Success: md 落到所选路径，toast 含「在文件夹中显示」
 *   ❌ Error: 目标目录不可写 → 明示 + 改路径重试（不生成半成品文件）
 *   ↩ Abandon: 编辑中途离开 → 草稿保留在编辑态（本地即时保存）
 */
import { useMemo, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Checkbox } from '@/components/ui/checkbox'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { seedTasks, lastWeekLogs, seedWeeklyDraft, TODAY } from '../shared/mock-data'

export default function Flow5WeeklyReview() {
  const [excluded, setExcluded] = useState<Set<string>>(new Set())
  const [exported, setExported] = useState<string | null>(null)
  const [pathRo, setPathRo] = useState(false)
  const [pathErr, setPathErr] = useState(false)
  const [emptyWeekDemo, setEmptyWeekDemo] = useState(false)

  const drafts = useMemo(
    () => seedWeeklyDraft.done.map((l) => ({ ...l, carriedOver: false })),
    [],
  )
  const rows = emptyWeekDemo ? [] : drafts
  const buckets = {
    完成: rows.length,
    顺延: seedWeeklyDraft.carriedOver.length,
    分类分布: Object.entries(emptyWeekDemo ? { 工作: 0, 学习: 0, 生活: 0 } : seedWeeklyDraft.categoryBreakdown).map(([k, v]) => `${k} ${v}`).join(' · '),
  }
  const included = rows.filter((r) => !excluded.has(r.taskId))

  const toggleExclude = (id: string, on: boolean) =>
    setExcluded((prev) => { const n = new Set(prev); if (on) n.delete(id); else n.add(id); return n })

  return (
    <div className="mx-auto max-w-lg space-y-4 pt-4">
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-semibold tracking-tight text-foreground">回顾 · 本周草稿
          {emptyWeekDemo && <span className="text-xs"> · 空周样例</span>}</h1>
        <Button size="sm" variant="ghost" className="h-6 text-[11px]" onClick={() => setEmptyWeekDemo((v) => !v)}>{emptyWeekDemo ? '看真实周' : '预览空周态'}</Button>
      </div>

      <Tabs defaultValue="weekly">
        <TabsList className="w-full">
          <TabsTrigger value="weekly" className="flex-1">周汇总草稿</TabsTrigger>
          <TabsTrigger value="log" className="flex-1">日志簿</TabsTrigger>
        </TabsList>

        {/* SCREEN 2 · 草稿 —— 三桶 + 勾选即编辑，明文可导 */}
        <TabsContent value="weekly" className="space-y-3">
          {rows.length === 0 ? (
            <Card><CardContent className="p-5 text-center text-sm text-muted-foreground">
              这周还没完成事项。完成一条就会自动进草稿；不想到时候手写，就保持「自动」开着。</CardContent></Card>
          ) : (<>
            <div className="flex gap-2 text-xs"><Badge variant="secondary" className="text-[11px]">完成 {buckets.完成}</Badge>
              <Badge variant="outline" className="text-[11px]">顺延 {buckets.顺延}</Badge>
              <Badge variant="outline" className="text-[11px]">{buckets.分类分布}</Badge></div>
            <Card><CardHeader className="pb-1"><CardTitle className="text-sm">周五草稿 · {seedWeeklyDraft.week}</CardTitle></CardHeader>
              <CardContent className="space-y-1 p-3">
                {['本周完成', '顺延未完成', '下周候选（拖留 ≥3 天）'].map((sec) => <div key={sec} className="text-[11px] text-muted-foreground">{sec}</div>)}
                {rows.map((r) => (
                  <label key={r.taskId} className="flex cursor-pointer items-center gap-3 rounded px-1 py-1.5 text-sm hover:bg-muted">
                    <Checkbox checked={!excluded.has(r.taskId)}
                      onCheckedChange={(c) => toggleExclude(r.taskId, c === true)} />
                    <span className={`flex-1 ${excluded.has(r.taskId) ? 'text-muted-foreground/60' : 'text-foreground'}`}>{r.title}</span>
                    <span className="font-mono text-[10px] text-muted-foreground">{r.doneAt.slice(11)}</span>
                  </label>
                ))}
              </CardContent></Card>
          </>)}
          <div className="flex flex-wrap items-center gap-3 text-xs text-muted-foreground">
            <label className="flex items-center gap-1"><input type="radio" checked={!pathRo} onChange={() => { setPathRo(false); setPathErr(false) }} />~/Docs/Kettd（可写）</label>
            <label className="flex items-center gap-1"><input type="radio" checked={pathRo} onChange={() => setPathRo(true)} />Z:\（只读演示）</label>
          </div>
          {pathErr && (
            // STATE: export-unwritable —— 失败不产生半成品；换路出口（edge must）
            <div className="flex items-center justify-between rounded-lg border border-destructive/40 bg-destructive/5 px-3 py-2 text-sm text-destructive">
              这个位置写不了（只读盘或被占用）
              <Button size="sm" variant="outline" className="h-7 text-xs" onClick={() => { setPathRo(false); setPathErr(false); setExported(`~/Docs/Kettd/${TODAY}-weekly-review.md（改位置后重试 · 共 ${included.length} 条）`) }}>换个位置 · 重试</Button>
            </div>
          )}
          <div className="flex gap-2">
            <Button size="sm" className="flex-1" onClick={() => { if (pathRo) { setExported(null); setPathErr(true) } else { setPathErr(false); setExported(`~/Docs/Kettd/${TODAY}-weekly-review.md（共 ${included.length} 条明细）`) } }}>导出 md</Button>
            <Button variant="outline" size="sm" className="flex-1" onClick={() => { if (pathRo) { setPathErr(true) } else { setPathErr(false); setExported(`~/Docs/Kettd/${TODAY}-weekly.csv`) } }}>导出 csv</Button>
          </div>
          {exported && (
            // SCREEN 3 · 导出结果：明文路径可见；失败态语义见 PRD 6.5 AC4
            <Card className="border-input bg-accent/50"><CardContent className="flex items-center justify-between gap-2 p-3 text-sm">
              <span className="truncate"><Badge variant="outline" className="mr-2 text-[10px]">✓ 已导出</Badge><code>{exported}</code></span>
              <Button size="sm" variant="ghost" className="h-6 shrink-0 text-[11px]">在文件夹中显示</Button>
            </CardContent></Card>
          )}
        </TabsContent>

        {/* SCREEN 1 · 日志簿：完成时刻 + 按日分组 + legacy（v1 迁移项）标记 */}
        <TabsContent value="log" className="space-y-3">
          {[[TODAY, seedTasks.filter((t) => t.done)], ['2026-09-01', []]].map(([day, list]) => (
            <div key={day as string}>
              <div className="font-mono text-xs text-muted-foreground">{day as string}{(list as typeof seedTasks).length ? '' : ' · 空日，如实留白'}</div>
              {(list as typeof seedTasks).map((t) => (
                <div key={t.id} className="mt-1 flex items-center gap-2 text-sm text-foreground">
                  <span aria-hidden>✓</span>{t.title}
                  <span className="ml-auto font-mono text-[10px] text-muted-foreground">{t.doneAt ?? ''}</span>
                </div>
              ))}
            </div>
          ))}
          <div className="border-t pt-3 text-xs text-muted-foreground">
            上周（按 {lastWeekLogs.length} 条完成 · legacy 项以虚线标记，导出时保留日期不留时刻）
          </div>
          {lastWeekLogs.slice(0, 4).map((l) => (
            <div key={l.taskId} className="mt-1 flex items-baseline justify-between text-xs">
              <span className="text-foreground">{l.title}</span>
              <span className="font-mono text-muted-foreground">{l.doneAt.slice(5, 10)} {l.doneAt.slice(11)}</span>
            </div>
          ))}
        </TabsContent>
      </Tabs>
    </div>
  )
}
