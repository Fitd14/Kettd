import { useEffect, useRef, useState } from 'react'
import type { BackupInfo, Bootstrap, HotkeyStatus, KbBackupInfo } from '@/lib/api'
import {
  clearEvents,
  createSticky,
  deleteSticky,
  getBackups,
  getHotkeyStatus,
  getKbItems,
  listKbBackups,
  listStickies,
  openDataFolder,
  openSticky,
  restoreBackup,
  restoreKbBackup,
  setSettings,
  updateSticky,
  type StickyNoteItem,
} from '@/lib/api'
import { STICKY_PAPERS } from '@/views/sticky-labels'
import { TimeText } from '@/components/time-text'
import './settings-sticky.css'
import './settings-backup.css'

interface Props {
  boot: Bootstrap
  refresh: () => Promise<void>
}

/** 与 src-tauri/src/models.rs DEFAULT_HOTKEY / DEFAULT_MAIN_HOTKEY / DEFAULT_STICKY_HOTKEY 保持一致 */
const DEFAULT_CAPTURE = 'Alt+Shift+A'
const DEFAULT_MAIN = 'Alt+Shift+O'
const DEFAULT_STICKY = 'Alt+Shift+S'
const DEFAULT_TODO = 'Alt+Shift+T'

/**
 * 可见性兼容读取（批次 3）：旧数据 / 旧桥接可能没有 visible 字段，
 * 缺失一律按「可见」（对齐 models.rs `#[serde(default = true)]`），避免把旧便签误标成已隐藏。
 */
const isStickyVisible = (st: StickyNoteItem & { visible?: boolean }) => st.visible !== false

/**
 * 自由便签列表预览：剥掉 Markdown 标记，取第一个非空的文本行，截 60 字。
 * 返回 '' 由调用方兜底「（空）」；入参防御 null/undefined（content 缺字段黑屏教训，2026-09-09）。
 */
function plainPreview(md: string | null | undefined): string {
  const raw = typeof md === 'string' ? md : ''
  const line = raw
    .split('\n')
    .map((l) =>
      l
        .replace(/^\s{0,3}#{1,6}\s+/, '') // 标题 #
        .replace(/^\s{0,3}>\s?/, '') // 引用 >
        .replace(/^\s*(?:[-*+]|\d+[.)])\s+/, '') // 列表符号
        .replace(/!?\[([^\]]*)\]\([^)]*\)/g, '$1') // 图片/链接 → 保留文字
        .replace(/[*_`~]/g, '') // 强调/代码标记
        .trim(),
    )
    .find((l) => l.length > 0)
  if (!line) return ''
  return line.length > 60 ? `${line.slice(0, 60)}…` : line
}

/**
 * 备份时间展示：后端给出的是本地语义串（`YYYY-MM-DDTHH:mm`，见 fs_store 的 list_backups），
 * 这里只把 T 换成空格便于阅读，禁止任何 UTC/时区换算（时间零 UTC 红线，models.rs）。
 */
function backupTimeText(value: string | null | undefined): string {
  const raw = typeof value === 'string' ? value : ''
  return raw ? raw.replace('T', ' ') : '未知时间'
}

/** 不可读备份的 error 文案：JSON 解析错误可能很长，截断展示、全文交给 title 提示 */
function backupErrText(value: string | null | undefined): string {
  const raw = typeof value === 'string' ? value.trim() : ''
  if (!raw) return '备份内容无法读取'
  return raw.length > 72 ? `${raw.slice(0, 72)}…` : raw
}

/** 设置视图（planned-settings-ui-spec）：分区块控件，改动即存（set_settings 增量补丁）。 */
export function SettingsView({ boot, refresh }: Props) {
  const s = boot.settings
  const [err, setErr] = useState<string | null>(null)
  const [note, setNote] = useState<string | null>(null)
  /** 便签区局部错误（新建/打开/删除失败 → 中文提示就近显示，不用 alert 丢信息） */
  const [stickyError, setStickyError] = useState<string | null>(null)
  const [hotkeys, setHotkeys] = useState<HotkeyStatus | null>(null)
  const [capCombo, setCapCombo] = useState(s.captureHotkey)
  const [mainCombo, setMainCombo] = useState(s.mainHotkey ?? '')
  const [stickyCombo, setStickyCombo] = useState(s.stickyHotkey ?? '')
  const [todoCombo, setTodoCombo] = useState(s.todoHotkey ?? '')
  const [fadeOpacity, setFadeOpacity] = useState(s.stickyFadeOpacity)
  const [confirmClear, setConfirmClear] = useState(false)
  const [confirmDel, setConfirmDel] = useState<string | null>(null)
  const [stickyList, setStickyList] = useState<StickyNoteItem[]>([])
  const [kbTitleMap, setKbTitleMap] = useState<Map<string, string>>(new Map())
  /** 备份与恢复区（批次 4 #7）：data.json 与 notes.json 各自动轮转 5 份 */
  const [taskBackups, setTaskBackups] = useState<BackupInfo[] | null>(null)
  const [kbBackups, setKbBackups] = useState<KbBackupInfo[] | null>(null)
  const [backupError, setBackupError] = useState<string | null>(null)
  const [kbBackupError, setKbBackupError] = useState<string | null>(null)
  const [backupNote, setBackupNote] = useState<string | null>(null)
  const [kbBackupNote, setKbBackupNote] = useState<string | null>(null)
  /** 两步确认：当前进入「确认恢复」的备份 slot（3 秒无操作自动回退） */
  const [confirmRestore, setConfirmRestore] = useState<string | null>(null)
  const [confirmKbRestore, setConfirmKbRestore] = useState<string | null>(null)
  /** 正在恢复的备份 slot（恢复中禁用按钮并显示 loading） */
  const [restoringSlot, setRestoringSlot] = useState<string | null>(null)
  const [restoringKbSlot, setRestoringKbSlot] = useState<string | null>(null)
  const clearTimer = useRef<number | null>(null)
  const backupTimer = useRef<number | null>(null)
  const kbBackupTimer = useRef<number | null>(null)

  // 可见 / 隐藏计数（批次 3：上限改为「可见便签合计 5」，不再按自由/知识分池）
  const visibleCount = stickyList.filter((s) => isStickyVisible(s)).length
  const hiddenCount = stickyList.length - visibleCount

  useEffect(() => {
    void getHotkeyStatus().then((r) => { if (!r.err && r.data) setHotkeys(r.data) })
    void loadStickies()
    void loadBackups()
    void loadKbBackups()
    void getKbItems().then((r) => {
      if (!r.err && r.data) setKbTitleMap(new Map(r.data.map((i) => [i.id, i.title])))
    })
  }, [])

  const loadStickies = async () => {
    const r = await listStickies()
    if (!r.err && r.data) setStickyList(r.data)
  }

  const loadBackups = async () => {
    const r = await getBackups()
    if (r.err) { setBackupError(r.err); return }
    setBackupError(null)
    setTaskBackups(r.data ?? [])
  }

  const loadKbBackups = async () => {
    const r = await listKbBackups()
    if (r.err) { setKbBackupError(r.err); return }
    setKbBackupError(null)
    setKbBackups(r.data ?? [])
  }

  // 滑杆本地值跟随后端（他窗改动/回滚时回正）
  useEffect(() => { setFadeOpacity(s.stickyFadeOpacity) }, [s.stickyFadeOpacity])

  useEffect(() => () => {
    if (clearTimer.current) window.clearTimeout(clearTimer.current)
    if (backupTimer.current) window.clearTimeout(backupTimer.current)
    if (kbBackupTimer.current) window.clearTimeout(kbBackupTimer.current)
  }, [])

  const patch = async (p: Parameters<typeof setSettings>[0]) => {
    const r = await setSettings(p)
    if (r.err) { setErr(r.err); return }
    setErr(null)
    await refresh()
  }

  const rebind = async (slot: 'capture' | 'main' | 'sticky' | 'todo') => {
    const r = slot === 'capture'
      ? await setSettings({ captureHotkey: capCombo.trim() })
      : slot === 'main'
        ? await setSettings({ mainHotkey: mainCombo.trim() || null })
        : slot === 'sticky'
          ? await setSettings({ stickyHotkey: stickyCombo.trim() || null })
          : await setSettings({ todoHotkey: todoCombo.trim() || null })
    if (r.err) { setErr(r.err); return }
    const st = await getHotkeyStatus()
    if (!st.err && st.data) setHotkeys(st.data)
    await refresh()
  }

  const doCreateSticky = async () => {
    const r = await createSticky()
    if (r.err) { setStickyError(r.err); return }
    setStickyError(null)
    void loadStickies()
    await refresh()
  }

  /** 重新打开被关闭（visible=false）的便签；可见上限 5，失败给出中文提示 */
  const doOpenSticky = async (st: StickyNoteItem) => {
    const r = await openSticky(st.id)
    if (r.err) { setStickyError(r.err); return }
    setStickyError(null)
    await loadStickies()
    await refresh()
  }

  const doToggleSticky = async (st: StickyNoteItem) => {
    const r = await updateSticky(st.id, { mini: !st.mini })
    if (r.err) { setStickyError(r.err); return }
    setStickyError(null)
    void loadStickies()
  }

  /** 删除不可恢复（两步确认）；隐藏便签即使窗口未打开也允许删除 */
  const doDeleteSticky = async (st: StickyNoteItem) => {
    const r = await deleteSticky(st.id)
    if (r.err) { setStickyError(r.err); return }
    setStickyError(null)
    setConfirmDel(null)
    await loadStickies()
    await refresh()
  }

  const restoreDefaults = async () => {
    const r = await setSettings({
      captureHotkey: DEFAULT_CAPTURE,
      mainHotkey: DEFAULT_MAIN,
      stickyHotkey: DEFAULT_STICKY,
      todoHotkey: DEFAULT_TODO,
    })
    if (r.err) { setErr(r.err); return }
    setCapCombo(DEFAULT_CAPTURE)
    setMainCombo(DEFAULT_MAIN)
    setStickyCombo(DEFAULT_STICKY)
    setTodoCombo(DEFAULT_TODO)
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

  /**
   * 从 data.json 备份恢复任务数据：两步内联确认（首次点击 →「确认恢复」，3 秒回退）。
   * 成功后重新拉备份列表并 refresh() 拉一次 bootstrap（任务/提醒/健康度一起更新），不整页重载。
   */
  const doRestoreBackup = async (info: BackupInfo) => {
    if (!info.readable || restoringSlot) return
    if (confirmRestore !== info.slot) {
      if (backupTimer.current) window.clearTimeout(backupTimer.current)
      setConfirmRestore(info.slot)
      setBackupNote(null)
      backupTimer.current = window.setTimeout(() => setConfirmRestore(null), 3000)
      return
    }
    if (backupTimer.current) window.clearTimeout(backupTimer.current)
    setConfirmRestore(null)
    setRestoringSlot(info.slot)
    setBackupError(null)
    setBackupNote(null)
    const r = await restoreBackup(info.slot)
    setRestoringSlot(null)
    if (r.err) { setBackupError(r.err); return }
    setBackupNote(`已恢复 ${r.data?.restored ?? 0} 条任务`)
    await loadBackups()
    await refresh()
  }

  /**
   * 从 notes.json 备份恢复知识库：同样两步确认；成功后重拉备份列表并触发数据刷新。
   */
  const doRestoreKbBackup = async (info: KbBackupInfo) => {
    if (!info.readable || restoringKbSlot) return
    if (confirmKbRestore !== info.slot) {
      if (kbBackupTimer.current) window.clearTimeout(kbBackupTimer.current)
      setConfirmKbRestore(info.slot)
      setKbBackupNote(null)
      kbBackupTimer.current = window.setTimeout(() => setConfirmKbRestore(null), 3000)
      return
    }
    if (kbBackupTimer.current) window.clearTimeout(kbBackupTimer.current)
    setConfirmKbRestore(null)
    setRestoringKbSlot(info.slot)
    setKbBackupError(null)
    setKbBackupNote(null)
    const r = await restoreKbBackup(info.slot)
    setRestoringKbSlot(null)
    if (r.err) { setKbBackupError(r.err); return }
    setKbBackupNote(`已恢复 ${r.data?.restored ?? 0} 条知识`)
    await loadKbBackups()
    await refresh()
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
        <div className="section-head"><b>便签</b><span>随手一张纸 · 常驻置顶 · 关闭后保留，可在下方重新打开</span></div>
        {stickyError && <div className="inline-err" role="alert">{stickyError}</div>}
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
          <div className="tiny text-muted">鼠标移出便签时淡至下方透明度，融入桌面仍可扫读（缩小条不淡化）</div>
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
        <div className="set-row"><span>新建便签
          <div className="tiny text-muted">随手写一张纸（≤500 字）· 可见 {visibleCount}/5 · 隐藏 {hiddenCount} · 热键 {stickyCombo.trim() || '（未绑定）'}</div>
        </span>
          <span className="row-flex items-center gap-1.5">
            <button className="btn xs outline" onClick={() => { void doCreateSticky() }}>新建便签</button>
          </span>
        </div>
        {stickyList.length > 0 && (
          <div className="set-row" style={{ alignItems: 'flex-start' }}><span>便签清单
            <div className="tiny text-muted">关闭只是隐藏，内容保留；删除才不可恢复。可见便签合计上限 5 张</div>
          </span>
            <span className="sticky-list">
              {stickyList.map((st) => {
                const visible = isStickyVisible(st)
                const preview = st.kbRef
                  ? `📎 ${kbTitleMap.get(st.kbRef) ?? '（知识条目已删除）'}`
                  : plainPreview(st.content) || '（空）'
                return (
                  <span key={st.id} className="sticky-row">
                    <span
                      className={`sticky-row-title${visible ? '' : ' is-hidden'}`}
                      title={preview}
                    >
                      {preview}
                    </span>
                    {!visible && <span className="badge sticky-badge-hidden">已隐藏</span>}
                    {st.mini && <span className="badge">已缩小</span>}
                    <button
                      className="btn ghost xs"
                      onClick={() => { void doOpenSticky(st) }}
                      aria-label={`打开便签：${preview}`}
                    >
                      打开
                    </button>
                    <button className="btn ghost xs" onClick={() => { void doToggleSticky(st) }}>{st.mini ? '展开' : '缩小'}</button>
                    {confirmDel === st.id ? (
                      <span className="row-flex items-center gap-1">
                        <button
                          className="btn xs danger"
                          onClick={() => { void doDeleteSticky(st) }}
                          title="删除后不可恢复；关闭便签只是隐藏"
                        >
                          确认删除
                        </button>
                        <button className="btn ghost xs" onClick={() => setConfirmDel(null)}>取消</button>
                      </span>
                    ) : (
                      <button
                        className="btn ghost xs"
                        onClick={() => setConfirmDel(st.id)}
                        title="删除后不可恢复；关闭便签只是隐藏"
                      >
                        删除
                      </button>
                    )}
                  </span>
                )
              })}
            </span>
          </div>
        )}
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
        <div className="set-row"><span>新建便签
          <div className="tiny text-muted">
            随手新建一张便签（不管主窗开没开）。当前绑定：{hotkeys?.sticky ?? '未绑定'}（留空 = 解绑）
          </div>
        </span>
          <span className="row-flex">
            <input className="input xs" value={stickyCombo} onChange={(e) => setStickyCombo(e.target.value)} aria-label="新建便签热键" />
            <button className="btn xs outline" onClick={() => { void rebind('sticky') }}>换绑</button>
          </span>
        </div>
        <div className="set-row"><span>待办悬浮窗
          <div className="tiny text-muted">
            呼出/隐藏今天待办的悬浮窗。当前绑定：{hotkeys?.todo ?? '未绑定'}（留空 = 解绑）
          </div>
        </span>
          <span className="row-flex">
            <input className="input xs" value={todoCombo} onChange={(e) => setTodoCombo(e.target.value)} aria-label="待办悬浮窗热键" />
            <button className="btn xs outline" onClick={() => { void rebind('todo') }}>换绑</button>
          </span>
        </div>
        <div className="set-row"><span>恢复默认
          <div className="tiny text-muted">快速记录 {DEFAULT_CAPTURE} · 打开主界面 {DEFAULT_MAIN} · 新建便签 {DEFAULT_STICKY} · 待办悬浮窗 {DEFAULT_TODO}</div>
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

      <section className="section" aria-label="备份与恢复">
        <div className="section-head">
          <b>备份与恢复</b>
          <span>写盘前自动轮转 5 份 · 恢复会用选中备份覆盖当前数据，操作不可撤销</span>
        </div>

        <div className="set-row" style={{ alignItems: 'flex-start' }}>
          <span>任务数据备份
            <div className="tiny text-muted">
              每次保存自动轮转 5 份（data.json.bak-1 … bak-5），恢复前自动轮转、不挤掉所选备份
            </div>
          </span>
          <span className="backup-list">
            {backupError && <span className="inline-err" role="alert">{backupError}</span>}
            {backupNote && !backupError && <span className="inline-note" role="status">{backupNote}</span>}
            {taskBackups === null && !backupError && <span className="tiny text-muted">读取中…</span>}
            {taskBackups !== null && taskBackups.length === 0 && (
              <span className="tiny text-muted">暂无备份</span>
            )}
            {taskBackups !== null && taskBackups.length > 0 && (
              <span className="backup-rows" role="list">
                {taskBackups.map((info) => {
                  const confirming = confirmRestore === info.slot
                  const busy = restoringSlot === info.slot
                  return (
                    <span key={info.slot} className="backup-row" role="listitem">
                      <span className="backup-row-main">
                        <b className="backup-slot">#{info.slot}</b>
                        <span className="backup-meta" title={info.name}>
                          {backupTimeText(info.createdAt)} · {info.sizeKb} KB · {info.taskCount} 条任务
                        </span>
                      </span>
                      {info.readable
                        ? <span className="badge">可读</span>
                        : <span className="badge backup-badge-bad">不可读</span>}
                      {!info.readable && (
                        <span className="backup-err" title={info.error ?? undefined}>
                          {backupErrText(info.error)}
                        </span>
                      )}
                      <button
                        className={`btn xs ${confirming ? 'danger' : 'outline'}`}
                        disabled={!info.readable || restoringSlot !== null}
                        aria-busy={busy}
                        aria-label={
                          !info.readable
                            ? `备份 ${info.slot} 不可读，无法恢复`
                            : busy
                              ? `正在从备份 ${info.slot} 恢复任务数据`
                              : confirming
                                ? `再次点击确认恢复备份 ${info.slot}，将覆盖当前任务数据`
                                : `从备份 ${info.slot} 恢复任务数据`
                        }
                        title={info.readable ? undefined : (info.error ?? '备份内容无法读取')}
                        onClick={() => { void doRestoreBackup(info) }}
                      >
                        {busy ? '恢复中…' : confirming ? '确认恢复' : '恢复'}
                      </button>
                    </span>
                  )
                })}
              </span>
            )}
          </span>
        </div>

        <div className="set-row" style={{ alignItems: 'flex-start' }}>
          <span>知识库备份
            <div className="tiny text-muted">
              每次保存知识库前自动轮转 5 份（notes.json.bak-1 … bak-5），恢复后覆盖当前知识库
            </div>
          </span>
          <span className="backup-list">
            {kbBackupError && <span className="inline-err" role="alert">{kbBackupError}</span>}
            {kbBackupNote && !kbBackupError && <span className="inline-note" role="status">{kbBackupNote}</span>}
            {kbBackups === null && !kbBackupError && <span className="tiny text-muted">读取中…</span>}
            {kbBackups !== null && kbBackups.length === 0 && (
              <span className="tiny text-muted">暂无备份</span>
            )}
            {kbBackups !== null && kbBackups.length > 0 && (
              <span className="backup-rows" role="list">
                {kbBackups.map((info) => {
                  const confirming = confirmKbRestore === info.slot
                  const busy = restoringKbSlot === info.slot
                  return (
                    <span key={info.slot} className="backup-row" role="listitem">
                      <span className="backup-row-main">
                        <b className="backup-slot">#{info.slot}</b>
                        <span className="backup-meta" title={info.name}>
                          {backupTimeText(info.createdAt)} · {info.sizeKb} KB · {info.noteCount} 条知识
                        </span>
                      </span>
                      {info.readable
                        ? <span className="badge">可读</span>
                        : <span className="badge backup-badge-bad">不可读</span>}
                      {!info.readable && (
                        <span className="backup-err" title={info.error ?? undefined}>
                          {backupErrText(info.error)}
                        </span>
                      )}
                      <button
                        className={`btn xs ${confirming ? 'danger' : 'outline'}`}
                        disabled={!info.readable || restoringKbSlot !== null}
                        aria-busy={busy}
                        aria-label={
                          !info.readable
                            ? `知识库备份 ${info.slot} 不可读，无法恢复`
                            : busy
                              ? `正在从知识库备份 ${info.slot} 恢复`
                              : confirming
                                ? `再次点击确认恢复知识库备份 ${info.slot}，将覆盖当前知识库`
                                : `从知识库备份 ${info.slot} 恢复`
                        }
                        title={info.readable ? undefined : (info.error ?? '备份内容无法读取')}
                        onClick={() => { void doRestoreKbBackup(info) }}
                      >
                        {busy ? '恢复中…' : confirming ? '确认恢复' : '恢复'}
                      </button>
                    </span>
                  )
                })}
              </span>
            )}
          </span>
        </div>
      </section>

      <div className="version-line text-muted">Kettd v{boot.version} · 离线保存</div>
    </div>
  )
}
