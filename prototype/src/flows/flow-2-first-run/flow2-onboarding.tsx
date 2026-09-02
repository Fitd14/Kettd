/*
 * FLOW: First-Run Lessons · 首启三课（story-3 ⭐ / PRD §6.2）
 * ENTRY: 首次安装启动（主窗隐藏后的默认落地）
 * SCREENS: 5（Welcome → ①记一条 → ②看今天 → ③收进托盘 → Done）
 * EXIT:
 *   ✅ Success: 三步完成，真实任务驻留今日，零假数据
 *   ❌ Error: 热键注册失败 → 「点这里代替按键」主按钮（hotkey-conflict，已接线）
 *   ↩ Abandon: 任一步跳过/关闭 → dismissed 持久（skip-persistence）；重开从断点续（resume-interrupted）
 */
import { useEffect, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Progress } from '@/components/ui/progress'
import { Switch } from '@/components/ui/switch'

const KEY_DISMISSED = 'kettd.onboarding.dismissed'
const KEY_STEP = 'kettd.onboarding.step' // 生产：两键迁移 Tauri store（防无痕环境丢进度）

const lsGet = (k: string) => { try { return localStorage.getItem(k) } catch { return null } }
const lsSet = (k: string, v: string) => { try { localStorage.setItem(k, v) } catch { /* 无痕/隔离环境忽略 */ } }

export default function Flow2FirstRun() {
  const dismissed = lsGet(KEY_DISMISSED) === '1'
  const [step, setStep] = useState(() => (dismissed ? 4 : Math.min(Math.max(Number(lsGet(KEY_STEP) ?? '0') || 0, 0), 4)))
  const [item, setItem] = useState('')
  const [tray, setTray] = useState(true)
  const [hotkeyFailed, setHotkeyFailed] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => { if (step > 0 && step < 4) lsSet(KEY_STEP, String(step)) }, [step])
  const dismiss = () => { lsSet(KEY_DISMISSED, '1'); lsSet(KEY_STEP, '4'); setStep(4) }

  return (
    <div className="mx-auto max-w-md space-y-4 pt-6">
      <div className="flex items-center justify-between text-xs text-muted-foreground">
        <span>{step === 4 && !dismissed ? '续上次 · 完成' : `首次启动 · 约 60 秒 · ${step}/3`}</span>
        <Button variant="ghost" size="sm" className="h-6 text-xs" onClick={dismiss}>全部跳过 / 不再提示</Button>
      </div>
      {step > 0 && step < 4 && <Progress value={(step / 3) * 100} aria-label="引导进度" />}

      {step === 0 && (
        <Card><CardHeader><CardTitle className="text-lg">把要记的事，交给一张安静的桌面纸片</CardTitle></CardHeader>
          <CardContent className="space-y-3">
            <p className="text-sm text-muted-foreground">1 分钟学会三件事。数据只存在这台电脑，没有账号，不会收费。</p>
            <Button className="w-full" onClick={() => setStep(1)}>{'开始第一课 · 记一条'}</Button>
          </CardContent></Card>
      )}

      {step === 1 && (
        // SCREEN · 实操捕获（强制成功才能下一步）
        <Card><CardHeader><CardTitle>{'第一课 · 随时一拍就记下'}</CardTitle></CardHeader>
          <CardContent className="space-y-3">
            {!hotkeyFailed ? (<>
              <p className="text-sm text-muted-foreground">以后在任何软件里按 <kbd className="rounded border px-1 font-mono text-xs">Alt+Shift+A</kbd> 就出输入条——现在先用手输一次：</p>
              <div className="flex gap-2">
                <Input autoFocus ref={inputRef} value={item} onChange={(e) => setItem(e.target.value)}
                  placeholder="例如：明早交周报" onKeyDown={(e) => e.key === 'Enter' && item.trim() && setStep(2)} />
                <Button disabled={!item.trim()} onClick={() => setStep(2)}>{'记下'}</Button>
              </div>
              <Button variant="link" size="sm" className="p-0 text-xs" onClick={() => setHotkeyFailed(true)}>快捷键被别的软件占了？点这里 →</Button>
            </>) : (
              // STATE: hotkey-conflict —— 免快捷键主路径（不是脚注）
              <div className="space-y-3">
                <div className="rounded-lg border border-input bg-accent/50 p-3 text-sm text-foreground">检测到快捷键没注册成 —— 已切到<b>免快捷键模式</b>：点下方输入框直接记（以后托盘菜单里也能随时唤起）。</div>
                <div className="flex gap-2">
                  <Input ref={inputRef} value={item} onChange={(e) => setItem(e.target.value)}
                    placeholder="例如：明早交周报" onKeyDown={(e) => e.key === 'Enter' && item.trim() && setStep(2)} />
                  <Button disabled={!item.trim()} onClick={() => setStep(2)}>{'记下'}</Button>
                </div>
                <Button variant="ghost" size="sm" className="h-6 text-[11px]" onClick={() => { setHotkeyFailed(false); setTimeout(() => inputRef.current?.focus(), 0) }}>重开快捷键演示</Button>
              </div>
            )}
          </CardContent></Card>
      )}

      {step === 2 && (
        <Card><CardHeader><CardTitle>{'第二课 · 挑出今天要做的'}</CardTitle></CardHeader>
          <CardContent className="space-y-2">
            <p className="mb-1 text-sm text-muted-foreground">「今天」不是到期筛选，是你挑出来的。刚记的这条已经在里面：</p>
            <Card><CardContent className="flex items-center justify-between p-3 text-sm">{item || '（刚才那条）'} <span className="text-xs text-muted-foreground">今天</span></CardContent></Card>
            <p className="text-[11px] text-muted-foreground">昨天没做完的会自动「拖到今天」并标记 —— 早上看一眼即可。</p>
            <Button className="w-full" onClick={() => setStep(3)}>{'学会 · 下一步'}</Button>
          </CardContent></Card>
      )}

      {step === 3 && (
        <Card><CardHeader><CardTitle>{'第三课 · 不用了就收进托盘'}</CardTitle></CardHeader>
          <CardContent className="space-y-3">
            <div className="flex items-center justify-between text-sm"><span>托盘常驻，桌面留一条小纸片（随时右键找回）</span><Switch checked={tray} onCheckedChange={setTray} /></div>
            <Button className="w-full" onClick={() => { lsSet(KEY_DISMISSED, '1'); setStep(4) }}>{'完成引导'}</Button>
          </CardContent></Card>
      )}

      {step === 4 && (
        // EXIT · Done（含 dismissed 后的直达态）
        <Card className="border-input bg-accent/50"><CardContent className="space-y-2 p-4 text-sm">
          <div className="font-medium">✓ 三步完成 —— 你已会用 80% 的 Kettd</div>
          <p className="text-muted-foreground">引导不会再出现。数据在本机 <code className="font-mono text-xs">%APPDATA%/todo-list</code>，出问题可一键回滚（设置 → 数据）。</p>
          <Button size="sm" className="mt-1">进今天 →</Button>
        </CardContent></Card>
      )}
    </div>
  )
}
