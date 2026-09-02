import { useState } from 'react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Input } from '@/components/ui/input'
import { parseCapture, TODAY } from './flows/shared/mock-data'

const FLOWS = [
  { id: 'flow-1', name: '3 秒捕获', prd: '§6.1 ⭐', done: false },
  { id: 'flow-2', name: '首启三课', prd: '§6.2 ⭐', done: false },
  { id: 'flow-3', name: '今日计划', prd: '§6.3 ⭐', done: false },
  { id: 'flow-4', name: '到点真提醒', prd: '§6.4 ⭐', done: false },
  { id: 'flow-5', name: '周汇总草稿', prd: '§6.5 ⭐·差异化主轴', done: false },
  { id: 'flow-6', name: '撤销与恢复', prd: '§6.6', done: false },
]

function App() {
  const [query, setQuery] = useState(`明天下午3点找导师 #学习 !高`)
  const result = parseCapture(query, TODAY)

  return (
    <div className="min-h-screen bg-background p-8">
      <header className="mb-6 max-w-4xl">
        <h1 className="text-xl font-semibold tracking-tight text-foreground">
          Kettd v2 原型 · 待 Phase B 接线
        </h1>
        <p className="text-sm text-muted-foreground">
          今日 {TODAY} · 下方为 story-1 解析器的实机预览，其余屏由 /Web页面设计 Phase B/C 接续
        </p>
      </header>

      <section className="mb-8 max-w-4xl">
        <Card>
          <CardHeader>
            <CardTitle className="text-base">快录解析预览（PRD §6.1 语法契约）</CardTitle>
          </CardHeader>
          <CardContent className="space-y-4">
            <Input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="明天下午3点找导师 #学习 !高"
              className="font-mono"
            />
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant="secondary">标题「{result.title || '…'}」</Badge>
              {result.chips.map((c) => (
                <Badge key={c.raw} className="bg-accent text-accent-foreground hover:bg-accent">
                  {c.value}
                </Badge>
              ))}
              {result.fellBack && <Badge variant="outline">解析为空 · 纯文本照收</Badge>}
            </div>
            <div className="font-mono text-xs text-muted-foreground">
              {'{ dueAt: '}{JSON.stringify(result.dueAt)}{', autoRemind: '}{String(result.autoRemind)}{' }'}
            </div>
          </CardContent>
        </Card>
      </section>

      <div className="grid max-w-4xl grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {FLOWS.map((f) => (
          <Card key={f.id}>
            <CardHeader className="pb-2">
              <CardTitle className="flex items-center justify-between text-sm">
                {f.name}
                <Badge variant="outline" className="font-mono text-[10px]">
                  {f.prd}
                </Badge>
              </CardTitle>
            </CardHeader>
            <CardContent className="text-xs text-muted-foreground">
              屏规格就绪 · mock 先行 · UI 待 Phase B
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  )
}

export default App
