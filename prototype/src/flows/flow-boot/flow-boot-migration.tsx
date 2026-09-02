/*
 * FLOW: Boot · v1 数据迁移兜底（edge: migration-v1-failed / PRD Gate 最高优先 must）
 * ENTRY: v2 首启读 v1 data.json 失败
 * SCREENS: 1（三选兜底 + 回退 v1 模式护资产）
 * 原则：绝不静默建空库；原文件永不删
 */
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'

export default function FlowBootMigration() {
  const [choice, setChoice] = useState<'idle' | 'empty-start' | 'v1-mode' | 'help'>('idle')
  return (
    <div className="mx-auto max-w-md space-y-4 pt-8">
      <Card className="border-destructive/40 shadow-lg">
        <CardHeader><CardTitle className="flex items-center justify-between text-base">
          旧数据没能搬过来，但它没丢 <Badge variant="outline" className="text-[10px]">data.v1.json</Badge></CardTitle></CardHeader>
        <CardContent className="space-y-4 text-sm">
          <p className="text-muted-foreground">v2 第一次启动时，原样读你的旧任务列表失败了 —— 我们<span className="text-foreground">没有</span>替你建一个空库，原文件一个字节都没改。</p>
          <div className="space-y-2">
            <Button className="w-full justify-start" variant={choice === 'v1-mode' ? 'secondary' : 'outline'} onClick={() => setChoice('v1-mode')}>用回 v1 模式继续阅读（推荐·保资产）</Button>
            <Button className="w-full justify-start" variant={choice === 'empty-start' ? 'secondary' : 'outline'} onClick={() => setChoice('empty-start')}>先用空 v2 记着，回头再修</Button>
            <Button className="w-full justify-start" variant={choice === 'help' ? 'secondary' : 'outline'} onClick={() => setChoice('help')}>发日志求助（含损坏文件头部快照）</Button>
          </div>
          {choice !== 'idle' && (
            <div className="rounded-lg bg-muted p-2.5 font-mono text-[10px] text-muted-foreground" role="status">
              {choice === 'v1-mode' && '→ 托盘提示「v2 待修复」；v1 数据窗口只读打开，写入排队到修复当日合并。'}
              {choice === 'empty-start' && '→ 空库 + 托盘黄点常驻：「有 1 份旧数据待恢复」，随时回到本屏重试；导出重建指引见 设置→数据。'}
              {choice === 'help' && '#LOG-SNAPSHOT: data.v1.json 前 64 字节 + 应用日志近 200 行（本机生成，不上传）。'}
            </div>
          )}
          <p className="text-[11px] text-muted-foreground">文件位置：<code>%APPDATA%/todo-list/</code> <Button size="sm" variant="link" className="h-4 p-0 align-baseline text-[11px]" onClick={() => { try { navigator.clipboard?.writeText('%APPDATA%/todo-list/') } catch { /* 隔离环境忽略 */ } }}>复制</Button></p>
        </CardContent>
      </Card>
    </div>
  )
}
