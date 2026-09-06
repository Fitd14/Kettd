import { Moon, Sun } from 'lucide-react'
import { useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

// M0 冒烟页：验证 (1) shadcn 组件可用 (2) Kettd 设计令牌生效 (3) 暗态可切 (4) Tailwind v4 工具类成图
export default function App() {
  const [dark, setDark] = useState(false)
  useEffect(() => {
    document.documentElement.classList.toggle('dark', dark)
  }, [dark])

  return (
    <div className="flex min-h-screen items-center justify-center bg-background p-6">
      <div className="w-full max-w-sm space-y-4 rounded-lg border border-border bg-card p-6 text-card-foreground shadow-md">
        <div className="space-y-1">
          <h1 className="text-lg font-semibold">Kettd · React 基座已就绪</h1>
          <p className="text-sm text-muted-foreground">
            M0 空壳双轨：vanilla 发布路径未动，此页仅验证新栈与令牌。
          </p>
        </div>

        <div className="flex flex-wrap gap-2">
          <Button variant="default">主按钮</Button>
          <Button variant="secondary">次按钮</Button>
          <Button variant="destructive">危险</Button>
          <Button variant="outline">描边</Button>
        </div>

        <div className="flex items-center gap-2 text-sm">
          <span className="rounded-pill bg-cat-life/15 px-2 py-0.5 text-cat-life">生活</span>
          <span className="rounded-pill bg-cat-study/15 px-2 py-0.5 text-cat-study">学习</span>
          <span className="rounded-pill bg-pri-high/15 px-2 py-0.5 text-pri-high">高优先</span>
          <span className="ml-auto text-xs text-muted-foreground">令牌与 vanilla 单一来源一致</span>
        </div>

        <Button
          variant="ghost"
          size="sm"
          className={cn('w-full justify-center gap-2')}
          onClick={() => setDark((v) => !v)}
        >
          {dark ? <Sun className="size-4" /> : <Moon className="size-4" />}
          切换到{dark ? '亮' : '暗'}色
        </Button>
      </div>
    </div>
  )
}
