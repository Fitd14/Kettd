import { useEffect, useRef, useState } from 'react'
import type { Bootstrap, HotkeyStatus } from '@/lib/api'
import {
  clearEvents,
  getHotkeyStatus,
  hideFloat,
  openDataFolder,
  setSettings,
  showFloat,
} from '@/lib/api'
import { STICKY_PAPERS } from '@/views/sticky-labels'
import { TimeText } from '@/components/time-text'

interface Props {
  boot: Bootstrap
  refresh: () => Promise<void>
}

/** 与 src-tauri/src/models.rs DEFAULT_HOTKEY / DEFAULT_MAIN_HOTKEY 保持一致 */
const DEFAULT_CAPTURE = 'Alt+Shift+A'
const DEFAULT_MAIN = 'Alt+Shift+O'

/** 设置视图（planned-settings-ui-spec）：分区块控件，改动即存（set_settings 增量补丁）。 */
export function SettingsView({ boot, refresh }: Props) {
  const s = boot.settings
  const [err, setErr] = useState<string | null>(null)
  const [note, setNote] = useState<string | null>(null)
  const [hotkeys, setHotkeys] = useState<HotkeyStatus | null>(null)
  const [capCombo, setCapCombo] = useState(s.captureHotkey)
  const [mainCombo, setMainCombo] = useState(s.mainHotkey ?? '')
  const [fadeOpacity, setFadeOpacity] = useState(s.stickyFadeOpacity)
  const [confirmClear, setConfirmClear] = useState(false)
  const clearTimer = useRef<number | null>(null)

  useEffect(() => {
    void getHotkeyStatus().then((r) => { if (!r.err && r.data) setHotkeys(r.data) })
  }, [])

  // 滑杆本地值跟随后端（他窗改动/回滚时回正）
  useEffect(() => { setFadeOpacity(s.stickyFadeOpacity) }, [s.stickyFadeOpacity])

  useEffect(() => () => {
    if (clearTimer.current) window.clearTimeout(clearTimer.current)
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

  const restoreDefaults = async () => {
    const r = await setSettings({ captureHotkey: DEFAULT_CAPTURE, mainHotkey: DEFAULT_MAIN })
    if (r.err) { setErr(r.err); return }
    setCapCombo(DEFAULT_CAPTURE)
    setMainCombo(DEFAULT_MAIN)
    const st = await getHotkeyStatus()
    if (!st.err && st.data) setHotkeys(st.data)
    setNote('已恢复默认快捷键')
    await refresh()
  }

  /** 清空本地统计：两步内联确认（Tauri WebView 无原生 confirm，禁用） */
  const doClearEvents = async () => {
    if (!confirmClear) {
      setConfirmClear(true)
      clearTimer.current = window.setTimeout(() => setConfirmClear(false), 3000)
      return
    }
    if (clearTimer.current) window.clearTimeout(clearTimer.current)
    setConfirmClear(false)
    const r = await clearEvents()
    if (r.err) { setErr(r.err); return }
    setErr(null)
    setNote('本地统计已清空')
  }

  return (
    <div className="view">
      <div>
        <div className="view-title">设置</div>
        <div className="view-sub">改动即存 · 控件皆有实行为（绑定失败会明示，不会假装成功）</div>
      </div>

      {err && <div className="inline-err" role="alert">{err}</div>}
      {note && !err && <div className="inline-note" role="status">{note}</div>}

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
      </section>

      <section className="section" aria-label="便签">
        <div className="section-head"><b>便签</b><span>单一便签 · 拖顶部胶条记位</span></div>
        <div className="set-row"><span>便签置顶
          <div className="tiny text-muted">固定 = 置顶；解除后便签沉到普通层，可被其他窗口盖住</div>
        </span>
          <div className="seg" role="group" aria-label="便签置顶">
            <button className={`btn sm ${s.stickyPinned ? 'on' : ''}`} onClick={() => { void patch({ stickyPinned: true }) }}>固定</button>
            <button className={`btn sm ${!s.stickyPinned ? 'on' : ''}`} onClick={() => { void patch({ stickyPinned: false }) }}>不固定</button>
          </div>
        </div>
        <div className="set-row"><span>纸面花纹
          <div className="tiny text-muted">水印级纹样，选墨竹/远山时折角旁伴一枚朱印</div>
        </span>
          <div className="seg" role="group" aria-label="纸面花纹">
            {[{ key: 'none', label: '无' }, { key: 'bamboo', label: '墨竹' }, { key: 'mountain', label: '远山' }].map((p) => (
              <button
                key={p.key}
                className={`btn sm ${s.stickyPattern === p.key ? 'on' : ''}`}
                onClick={() => { void patch({ stickyPattern: p.key }) }}
              >
                {p.label}
              </button>
            ))}
          </div>
        </div>
        <div className="set-row"><span>移出淡化
          <div className="tiny text-muted">鼠标移出便签时淡至下方透明度，融入桌面仍可扫读</div>
        </span>
          <span className="row-flex items-center gap-2">
            <div className="seg" role="group" aria-label="移出淡化">
              <button className={`btn sm ${s.stickyFade ? 'on' : ''}`} onClick={() => { void patch({ stickyFade: true }) }}>开</button>
              <button className={`btn sm ${!s.stickyFade ? 'on' : ''}`} onClick={() => { void patch({ stickyFade: false }) }}>关</button>
            </div>
            <input
              className="fade-slider"
              type="range"
              min={10}
              max={100}
              step={1}
              value={fadeOpacity}
              aria-label="淡化透明度百分比"
              aria-disabled={!s.stickyFade}
              disabled={!s.stickyFade}
              onChange={(e) => setFadeOpacity(Number(e.target.value))}
              onPointerUp={() => { void patch({ stickyFadeOpacity: fadeOpacity }) }}
              onKeyUp={(e) => { if (['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(e.key)) { void patch({ stickyFadeOpacity: fadeOpacity }) } }}
              onBlur={() => { if (fadeOpacity !== s.stickyFadeOpacity) { void patch({ stickyFadeOpacity: fadeOpacity }) } }}
            />
            <code className="tmeta mono" style={{ width: 34, textAlign: 'right' }}>{fadeOpacity}%</code>
          </span>
        </div>
        <div className="set-row"><span>显示 / 收起
          <div className="tiny text-muted">收起 = 收进托盘，进程常驻（便签规格 §4）</div>
        </span>
          <span className="row-flex items-center gap-1.5">
            <button className="btn xs outline" onClick={() => { void showFloat() }}>显示便签</button>
            <button className="btn xs outline" onClick={() => { void hideFloat() }}>收起便签</button>
          </span>
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
        <div className="set-row"><span>恢复默认
          <div className="tiny text-muted">快速记录 {DEFAULT_CAPTURE} · 打开主界面 {DEFAULT_MAIN}</div>
        </span>
          <button className="btn xs outline" onClick={() => { void restoreDefaults() }}>恢复默认快捷键</button>
        </div>
      </section>

      <section className="section" aria-label="提醒">
        <div className="section-head"><b>提醒</b></div>
        <div className="set-row"><span>免打扰
          <div className="tiny text-muted">期间静默入账，结束后合并补一条</div>
        </span>
          <span className="row-flex items-center gap-1.5">
            <div className="seg" role="group" aria-label="免打扰">
              <button className={`btn sm ${s.dnd.enabled ? 'on' : ''}`} onClick={() => { void patch({ dnd: { enabled: true } }) }}>开</button>
              <button className={`btn sm ${!s.dnd.enabled ? 'on' : ''}`} onClick={() => { void patch({ dnd: { enabled: false } }) }}>关</button>
            </div>
            <TimeText
              value={s.dnd.from}
              ariaLabel="免打扰开始时刻"
              onCommit={(v) => { void patch({ dnd: { from: v } }) }}
            />
            <span className="text-muted">–</span>
            <TimeText
              value={s.dnd.to}
              ariaLabel="免打扰结束时刻"
              onCommit={(v) => { void patch({ dnd: { to: v } }) }}
            />
          </span>
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
          <div className="tiny text-muted">仅本机统计，绝不出网 · 用于验证功能是否真被用上</div>
        </span>
          <span className="row-flex items-center gap-1.5">
            <div className="seg" role="group" aria-label="本地度量">
              <button className={`btn sm ${s.telemetryEnabled ? 'on' : ''}`} onClick={() => { void patch({ telemetryEnabled: true }) }}>开</button>
              <button className={`btn sm ${!s.telemetryEnabled ? 'on' : ''}`} onClick={() => { void patch({ telemetryEnabled: false }) }}>关</button>
            </div>
            <button
              className={`btn xs outline${confirmClear ? ' danger' : ''}`}
              aria-label={confirmClear ? '再次点击确认清空本地统计' : '清空本地统计'}
              onClick={() => { void doClearEvents() }}
            >
              {confirmClear ? '确认清空？' : '清空本地统计'}
            </button>
          </span>
        </div>
      </section>

      <div className="version-line text-muted">Kettd v{boot.version} · 离线优先 · 数据只在本机</div>
    </div>
  )
}
