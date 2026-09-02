/*
 * FLOW: Global Search（story-2 / PRD §6.7）—— overlay://search
 * ENTRY: 列表获焦即打字 · '/' · 侧栏搜索钮
 * SCREENS: 2 态（打字中骨架 / 结果分组 / 未找到+出路；筛选无结果=清筛选，词不同）
 * EXIT: Esc 恢复原焦点；回车直达该条（深链 ?open=）
 */
import { useEffect, useMemo, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent } from '@/components/ui/card'
import { seedTasks, lastWeekLogs, TODAY } from '../shared/mock-data'

type Group = 'doing' | 'log'
export default function FlowSearchOverlay() {
  const [q, setQ] = useState('复盘')
  const [debounced, setDebounced] = useState(q)
  const [busy, setBusy] = useState(false)
  const [filterDemo, setFilterDemo] = useState(false)

  useEffect(() => {
    setBusy(true)
    const t1 = setTimeout(() => setDebounced(q), 150) // debounce 150ms
    // 本地检索真实 <5ms：仅开「慢检索骨架」演示时才有可见骨架（loading-results）
    return () => clearTimeout(t1)
  }, [q])
  const [slowOn, setSlowOn] = useState(false)
  useEffect(() => { if (!busy) return; const t = setTimeout(() => setBusy(false), slowOn ? 360 : 0); return () => clearTimeout(t) }, [busy, slowOn])

  const hits = useMemo(() => {
    const term = debounced.trim()
    if (!term) return [] as { group: Group; id: string; title: string; due: string }[]
    const doing = seedTasks.filter((t) => !t.done && (t.title.includes(term) || (t.note ?? '').includes(term)))
      .map((t) => ({ group: 'doing' as Group, id: t.id, title: t.title, due: t.dueAt ?? '无截止' }))
    if (filterDemo) return doing // 筛选演示态：只看进行中，可能为 0→「清除筛选」出路
    const log = lastWeekLogs.filter((l) => l.title.includes(term))
      .map((l) => ({ group: 'log' as Group, id: l.taskId, title: l.title, due: `✓ ${l.doneAt.slice(0, 10)}` }))
    return [...doing, ...log]
  }, [debounced, filterDemo])

  return (
    <div className="mx-auto max-w-md space-y-3 pt-6" role="search">
      <div className="relative">
        <Input autoFocus value={q} onChange={(e) => setQ(e.target.value)}
          onKeyDown={(e) => { if (e.key === 'Escape') setQ('') }}
          placeholder="打字即搜 · Esc 关" className="font-mono" />
        <div className="absolute right-2 top-1/2 -translate-y-1/2">
          <Button size="sm" variant={slowOn ? 'secondary' : 'ghost'} className="h-6 text-[10px]" onClick={() => setSlowOn((v) => !v)}>{slowOn ? '慢检索中' : '演示慢检索骨架'}</Button>
        </div>
      </div>
      {busy && debounced.trim() ? (
        <div className="space-y-2" role="status"><div className="h-9 animate-pulse rounded-lg bg-muted" /><div className="h-9 w-4/5 animate-pulse rounded-lg bg-muted" /></div>
      ) : hits.length === 0 ? (
        filterDemo ? (
          <div className="rounded-lg border border-dashed p-4 text-center text-sm text-muted-foreground">筛选后没有 · <Button size="sm" variant="link" className="h-4 p-0 text-xs" onClick={() => setFilterDemo(false)}>清除筛选</Button></div>
        ) : (
          <div className="rounded-lg border border-dashed p-4 text-center text-sm text-muted-foreground">
            没找到「{debounced}」<div className="mt-1 text-xs">换个词 · 完成过的在「回顾·日志簿」也搜得到 · 一条没记过？<Button size="sm" variant="link" className="h-4 p-0 align-baseline text-xs">Alt+Shift+A 记一条</Button></div>
          </div>
        )
      ) : (
        <>
          {(['doing', 'log'] as Group[]).map((g) => {
            const grp = hits.filter((h) => h.group === g)
            if (!grp.length) return null
            return (
              <div key={g}>
                <div className="mb-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">{g === 'doing' ? '进行中' : '日志（完成）'}</div>
                {grp.map((h) => (
                  <Card key={h.id}><CardContent className="flex items-center justify-between gap-2 p-3 text-sm">
                    <span className="min-w-0 flex-1 truncate"><Mark text={h.title} term={debounced} /></span>
                    <code className="shrink-0 font-mono text-[10px] text-muted-foreground">{h.due}</code>
                    <Badge variant="outline" className="shrink-0 text-[10px]">Enter 直达</Badge>
                  </CardContent></Card>
                ))}
              </div>
            )
          })}
          <div className="text-right"><Button size="sm" variant="ghost" className="h-6 text-[10px]" onClick={() => setFilterDemo((v) => !v)}>{filterDemo ? '关闭筛选演示' : '演示：只看未完成的筛选空态'}</Button><span className="ml-2 text-[10px] text-muted-foreground">今日 · {TODAY}</span></div>
        </>
      )}
    </div>
  )
}

function Mark({ text, term }: { text: string; term: string }) {
  if (!term) return <>{text}</>
  const i = text.indexOf(term)
  if (i < 0) return <>{text}</>
  return (<>{text.slice(0, i)}<mark className="bg-accent text-accent-foreground">{term}</mark>{text.slice(i + term.length)}</>)
}
