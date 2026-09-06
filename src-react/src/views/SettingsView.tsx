import { useEffect, useState } from 'react'
import type { Bootstrap, HotkeyStatus } from '@/lib/api'
import {
  getHotkeyStatus,
  openDataFolder,
  setSettings,
} from '@/lib/api'
import { STICKY_PAPERS } from '@/views/sticky-labels'

interface Props {
  boot: Bootstrap
  refresh: () => Promise<void>
}

/** 设置视图（planned-settings-ui-spec + 便签规格 §12.1）：分区块控件，改动即存（set_settings 增量补丁）。 */
export function SettingsView({ boot, refresh }: Props) {
  const s = boot.settings
  const [err, setErr] = useState<string | null>(null)
  const [hotkeys, setHotkeys] = useState<HotkeyStatus | null>(null)
  const [capCombo, setCapCombo] = useState(s.captureHotkey)
  const [mainCombo, setMainCombo] = useState(s.mainHotkey ?? '')

  useEffect(() => {
    void getHotkeyStatus().then((r) => { if (!r.err && r.data) setHotkeys(r.data) })
  }, [])

  const patch = async (p: Parameters<typeof setSettings>[0]) => {
    const r = await setSettings(p)
    if (r.err) { setErr(r.err); return }
    setErr(null)
    await refresh()
  }

  const rebind = async (slot: 'capture' | 'main') => {
    const r = slot === 'capture'
      ? await setSettings({ captureHotkey: capCombo.trim() })
      : await setSettings({ mainHotkey: mainCombo.trim() || null })
    if (r.err) { setErr(r.err); return }
    const st = await getHotkeyStatus()
    if (!st.err && st.data) setHotkeys(st.data)
    await refresh()
  }

  return (
    <div className="view">
      <div>
        <div className="view-title">设置</div>
        <div className="view-sub">改动即存 · 控件皆有实行为（绑定失败会明示，不会假装成功）</div>
      </div>

      {err && <div className="inline-err" role="alert">{err}</div>}

      <section className="section" aria-label="外观">
        <div className="section-head"><b>外观</b></div>
        <div className="set-row"><span>主题</span>
          <div className="seg" role="group" aria-label="主题">
            <button className={`btn sm ${s.theme !== 'dark' ? 'on' : ''}`} onClick={() => { void patch({ theme: 'light' }) }}>纸白</button>
            <button className={`btn sm ${s.theme === 'dark' ? 'on' : ''}`} onClick={() => { void patch({ theme: 'dark' }) }}>暗色</button>
          </div>
        </div>
        <div className="set-row"><span>便签纸色
          <div className="tiny text-muted">便签规格 §12.1：预设四选一，分类色不随纸色变</div>
        </span>
          <div className="seg" role="group" aria-label="便签纸色">
            {STICKY_PAPERS.map((p) => (
              <button
                key={p.key}
                className={`btn sm ${s.stickyPaper === p.key ? 'on' : ''}`}
                onClick={() => { void patch({ stickyPaper: p.key }) }}
              >
                <span className={`paper-dot paper-${p.key}`} aria-hidden />
                {p.label}
              </button>
            ))}
          </div>
        </div>
        <div className="set-row"><span>便签置顶
          <div className="tiny text-muted">关闭后便签沉到普通层，可被其他窗口盖住</div>
        </span>
          <div className="seg" role="group" aria-label="便签置顶">
            <button className={`btn sm ${s.stickyPinned ? 'on' : ''}`} onClick={() => { void patch({ stickyPinned: true }) }}>固定</button>
            <button className={`btn sm ${!s.stickyPinned ? 'on' : ''}`} onClick={() => { void patch({ stickyPinned: false }) }}>不固定</button>
          </div>
        </div>
      </section>

      <section className="section" aria-label="快捷键">
        <div className="section-head"><b>快捷键</b><span>{hotkeys ? '以下为实际注册快照' : '读取中…'}</span></div>
        <div className="set-row"><span>快速记录
          <div className="tiny text-muted">
            当前绑定：{hotkeys?.capture ?? '未生效'}
            {hotkeys && !hotkeys.capture && '（被占用或未注册，可改后重试）'}
          </div>
        </span>
          <span className="row-flex">
            <input className="input xs" value={capCombo} onChange={(e) => setCapCombo(e.target.value)} aria-label="快速记录热键" />
            <button className="btn xs outline" onClick={() => { void rebind('capture') }}>换绑</button>
          </span>
        </div>
        <div className="set-row"><span>打开主界面
          <div className="tiny text-muted">当前绑定：{hotkeys?.main ?? '未绑定'}（留空 = 解绑）</div>
        </span>
          <span className="row-flex">
            <input className="input xs" value={mainCombo} onChange={(e) => setMainCombo(e.target.value)} aria-label="主界面热键" />
            <button className="btn xs outline" onClick={() => { void rebind('main') }}>换绑</button>
          </span>
        </div>
      </section>

      <section className="section" aria-label="提醒">
        <div className="section-head"><b>提醒</b></div>
        <div className="set-row"><span>免打扰
          <div className="tiny text-muted">{s.dnd.from} – {s.dnd.to} 期间静默入账，结束后合并补一条</div>
        </span>
          <div className="seg" role="group" aria-label="免打扰">
            <button className={`btn sm ${s.dnd.enabled ? 'on' : ''}`} onClick={() => { void patch({ dnd: { enabled: true } }) }}>开</button>
            <button className={`btn sm ${!s.dnd.enabled ? 'on' : ''}`} onClick={() => { void patch({ dnd: { enabled: false } }) }}>关</button>
          </div>
        </div>
        <div className="set-row"><span>每小时上限
          <div className="tiny text-muted">超出排队等下个窗口，绝不轰炸</div>
        </span>
          <div className="seg" role="group" aria-label="每小时提醒上限">
            {[3, 5, 10].map((n) => (
              <button
                key={n}
                className={`btn sm ${s.remindCapPerHour === n ? 'on' : ''}`}
                onClick={() => { void patch({ remindCapPerHour: n }) }}
              >
                {n} 条
              </button>
            ))}
          </div>
        </div>
      </section>

      <section className="section" aria-label="数据">
        <div className="section-head"><b>数据</b><span>全在本机 · 不出网</span></div>
        <div className="set-row"><span>数据健康
          <div className="tiny text-muted">
            {boot.health.health === 'ok'
              ? `正常 · 备份 ${boot.health.backups.length} 份可回滚`
              : `${boot.health.health} · ${boot.health.lastError ?? ''}`}
          </div>
        </span>
          <button className="btn xs outline" onClick={() => { void openDataFolder() }}>打开数据文件夹</button>
        </div>
        <div className="set-row"><span>本地度量
          <div className="tiny text-muted">纯本机统计（绝不出网），用于验证功能是否真被用上</div>
        </span>
          <div className="seg" role="group" aria-label="本地度量">
            <button className={`btn sm ${s.telemetryEnabled ? 'on' : ''}`} onClick={() => { void patch({ telemetryEnabled: true }) }}>开</button>
            <button className={`btn sm ${!s.telemetryEnabled ? 'on' : ''}`} onClick={() => { void patch({ telemetryEnabled: false }) }}>关</button>
          </div>
        </div>
      </section>
    </div>
  )
}
