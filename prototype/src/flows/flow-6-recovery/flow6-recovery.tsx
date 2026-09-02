/*
 * FLOW: Undo & Recovery · 撤销与恢复（story-8 底座 / PRD §6.6）
 * ENTRY: 任意删除之后 · 启动时 data.json 解析失败 · 任意写盘 IO 错误
 * SCREENS: 3 —— ①删除+10s 撤销条（倒计时可见）②损坏恢复页（绝不显示空应用）③写失败重试
 * EXIT:
 *   ✅ Success: 撤销原样回归（子任务/备注/计划）；回滚成功带损坏件留存路径；重试后真实写入
 *   ❌ Error: 备份全不可读 → 明文指引（文件位置 + 从导出重建）
 *   ↩ Abandon: 撤销窗口过期 → 回收站入口兜底（30 天自动清理）
 */
import { useEffect, useState, type ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Progress } from '@/components/ui/progress'
import { Separator } from '@/components/ui/separator'
import { seedTasks, seedBackups } from '../shared/mock-data'
import type { Task } from '../shared/types'
import type { DataHealth } from '../shared/types'

export default function Flow6Recovery() {
  const [tasks, setTasks] = useState<Task[]>(seedTasks.slice(0, 3))
  const [undoWin, setUndoWin] = useState<{ id: string; left: number } | null>(null)
  const [health, setHealth] = useState<DataHealth>('ok')
  const [writeFail, setWriteFail] = useState(false)
  const [allBackupsDead, setAllBackupsDead] = useState(false)

  const remove = (id: string) => {
    const gone = tasks.find((t) => t.id === id)
    setTasks((p) => p.filter((t) => t.id !== id))
    if (gone) setUndoWin({ id, left: 10 })
  }
  useEffect(() => {
    if (!undoWin || undoWin.left <= 0) return
    const it = setTimeout(() => setUndoWin((u) => (u ? { ...u, left: u.left - 1 } : null)), 1000)
    return () => clearTimeout(it)
  }, [undoWin])

  const undo = () => {
    if (!undoWin) return
    const src = seedTasks.find((t) => t.id === undoWin.id)
    if (src) setTasks((p) => [src, ...p])
    setUndoWin(null)
  }

  return (
    <div className="mx-auto max-w-md space-y-4 pt-4">
      {/* SCREEN 1 · 删除 → 撤销条 */}
      <h1 className="text-base font-semibold text-foreground">任务列表（误删演练场）</h1>
      {tasks.map((t) => (
        <Card key={t.id}><CardContent className="flex items-center gap-2 p-3 text-sm">
          <span className="flex-1 truncate">{t.title}</span>
          <Button size="sm" variant="ghost" className="h-6 text-[11px] text-destructive" onClick={() => remove(t.id)}>删除</Button>
        </CardContent></Card>
      ))}
      {tasks.length === 0 && (
        <Card><CardContent className="p-4 text-sm text-muted-foreground">
          都删完了 —— 别慌，下面撤销条与回收站才是这屏的主角。</CardContent></Card>
      )}
      {undoWin && (
        <Card className="border-input bg-muted"><CardContent className="p-3">
          <div className="flex items-center justify-between text-sm">
            <Span>已删除 <b>{seedTasks.find((t) => t.id === undoWin.id)?.title}</b></Span>
            <Button size="sm" onClick={undo} className="h-7 text-xs">撤销</Button>
          </div>
          <Progress value={undoWin.left * 10} className="mt-2 h-1" aria-label="撤销倒计时" />
          <div className="mt-1 text-right text-[10px] text-muted-foreground">{undoWin.left}s · 过期后进回收站（30 天）</div>
        </CardContent></Card>
      )}

      {/* STATE 切换：ok | corrupt | writeFailed */}
      <Separator />
      <div className="flex gap-2 text-xs">
        <Button size="sm" variant="outline" onClick={() => { setHealth('corrupt'); setWriteFail(false) }}>模拟：data.json 损坏</Button>
        <Button size="sm" variant="outline" onClick={() => { setHealth('ok'); setWriteFail(!writeFail) }}>模拟：写盘失败</Button>
        <Button size="sm" variant="ghost" className="text-[11px]" onClick={() => setAllBackupsDead((v) => !v)}>{allBackupsDead ? '恢复备份可读' : '再狠一点：备份全坏'}</Button>
      </div>

      {/* SCREEN 3 · 写失败 —— 不假装成功，也不丢输入 */}
      {writeFail && health === 'ok' && (
        <Card className="border-destructive/40"><CardContent className="flex items-center justify-between p-3 text-sm">
          <span className="text-destructive">没有保存 —— 磁盘拒绝写入（内容已保留在本机内存）</span>
          <div className="flex gap-1"><Button size="sm" variant="destructive" className="h-7 text-xs" onClick={() => setWriteFail(false)}>重试</Button>
            <Button size="sm" variant="ghost" className="h-7 text-xs" onClick={() => setHealth('corrupt')}>查看恢复页</Button></div>
        </CardContent></Card>
      )}

      {/* SCREEN 2 · 损坏恢复页 —— v1 病灶的反面：绝不出现"空应用" */}
      {health === 'corrupt' && !allBackupsDead && (
        <Card className="border-destructive/40 shadow-lg">
          <CardHeader><CardTitle className="flex items-center justify-between text-sm">
            无法读取数据文件 <Badge variant="destructive" className="text-[10px]">data.json 损坏</Badge></CardTitle></CardHeader>
          <CardContent className="space-y-3 text-sm">
            <p className="text-muted-foreground">这不是「你还没有任务」——文件读坏了，而且<em>有可用的昨日备份</em>。</p>
            <div className="space-y-1">
              {seedBackups.map((b) => (
                <div key={b.id} className="flex items-center justify-between rounded-md border border-input px-3 py-2">
                  <span className="text-xs"><b className="mr-2">{b.createdAt}</b><span className="text-muted-foreground">{b.sizeKB}KB · {b.taskCount} 条</span></span>
                  <Button size="sm" variant="outline" className="h-7 text-xs" onClick={() => setHealth('ok')}>恢复这份</Button>
                </div>
              ))}
            </div>
            <p className="text-[11px] text-muted-foreground">恢复前损坏文件会另存 <code className="font-mono">data.corrupt.时间戳</code>；全部不可读时，这里会给出文件位置与「从导出重建」指引。</p>
            <Button size="sm" variant="ghost" className="h-7 text-xs" onClick={() => setHealth('ok')}>我稍后处理 →</Button>
          </CardContent>
        </Card>
      )}
      {health === 'corrupt' && allBackupsDead && (
        <Card className="border-destructive/40 shadow-lg">
          <CardHeader><CardTitle className="text-sm">连自动备份都读不了 —— 你的文件原样躺着，没被动过</CardTitle></CardHeader>
          <CardContent className="space-y-3 text-sm">
            <div className="grid grid-cols-3 gap-2 text-xs">
              <Button variant="outline" size="sm" className="h-9 flex-col">打开数据文件夹</Button>
              <Button variant="outline" size="sm" className="h-9 flex-col">选任意备份文件恢复</Button>
              <Button variant="outline" size="sm" className="h-9 flex-col">从最近导出重建</Button>
            </div>
            <div className="rounded-md bg-muted p-2 font-mono text-[10px] text-muted-foreground">%APPDATA%/todo-list/data.corrupt.2026-09-02T1555<Button size="sm" variant="link" className="h-4 p-0 align-baseline text-[10px]" onClick={() => { try { navigator.clipboard?.writeText('%APPDATA%/todo-list') } catch { /* 无授权环境忽略 */ } }}>复制路径</Button></div>
            <p className="text-[11px] text-muted-foreground">求助可带这份路径与日志；应用不会替你“重置成空任务”。</p>
            <Button size="sm" variant="ghost" className="h-6 text-[11px]" onClick={() => { setHealth('ok'); setAllBackupsDead(false) }}>我稍后处理 →</Button>
          </CardContent>
        </Card>
      )}
    </div>
  )
}

function Span({ children }: { children: ReactNode }) {
  return <span className="truncate text-muted-foreground">{children}</span>
}
