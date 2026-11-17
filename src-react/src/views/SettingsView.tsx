import { useEffect, useRef, useState } from 'react'
import type {
  BackupExportResult,
  BackupImportReport,
  BackupInfo,
  Bootstrap,
  HotkeyStatus,
  KbBackupInfo,
  ReadingFontInfo,
  ReadingThemeInfo,
} from '@/lib/api'
import {
  clearEvents,
  createSticky,
  deleteReadingFont,
  deleteReadingTheme,
  deleteSticky,
  exportBackup,
  getBackups,
  getHotkeyStatus,
  getKbItems,
  importBackup,
  listKbBackups,
  listReadingFonts,
  listReadingThemes,
  listStickies,
  openDataFolder,
  openSticky,
  readReadingTheme,
  registerKbHotkey,
  restoreBackup,
  restoreKbBackup,
  saveReadingFont,
  saveReadingTheme,
  setSettings,
  updateSticky,
  type StickyNoteItem,
} from '@/lib/api'
import { READING_FONT_SYSTEMS } from '@/lib/reading-font'
import { parseTheme, BUILTIN_THEME_LABELS } from '@/lib/reading-theme'
import { BUILTIN_THEME_CSS } from '@/lib/reading-theme-source'

import { plainPreview } from '@/lib/sticky-text'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Dialog, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog'
import { STICKY_PAPERS } from '@/views/sticky-labels'
import './settings-sticky.css'
import './settings-backup.css'
import './settings-font.css'

interface Props {
  boot: Bootstrap
  refresh: () => Promise<void>
}

/** 与 src-tauri/src/models.rs DEFAULT_HOTKEY / DEFAULT_MAIN_HOTKEY / DEFAULT_STICKY_HOTKEY / DEFAULT_KB_HOTKEY 保持一致 */
const DEFAULT_CAPTURE = 'Alt+Shift+A'
const DEFAULT_MAIN = 'Alt+Shift+O'
const DEFAULT_STICKY = 'Alt+Shift+S'
const DEFAULT_TODO = 'Alt+Shift+T'
/** kb-storage-backup D6：直达知识库（隐藏主窗时唤出并停在 #/kb） */
const DEFAULT_KB = 'Alt+Shift+K'

/** 内置主题源码表由 `lib/reading-theme-source` 统一提供（与主窗/三窗共用一份） */

/**
 * 可见性兼容读取（批次 3）：旧数据 / 旧桥接可能没有 visible 字段，
 * 缺失一律按「可见」（对齐 models.rs `#[serde(default = true)]`），避免把旧便签误标成已隐藏。
 */
const isStickyVisible = (st: StickyNoteItem & { visible?: boolean }) => st.visible !== false

/** 便签预览统一走 lib/sticky-text（卡片墙同口径；空内容由调用方兜底「（空）」） */

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

/* ───────── 手动备份（kb-storage-backup 4.4 · D3/D4）───────── */

/** 前端上限（D4）：备份文件超过即拒绝，连读全文都省掉；后端另有格式/大小校验 */
const BACKUP_FILE_MAX_BYTES = 256 * 1024 * 1024
/** 预览里冲突标题最多直接列出的条数，超出折叠成「等 N 条」 */
const CONFLICT_TITLE_PREVIEW = 5

/** 导入完成后的一句话结果（口径同后端 ImportReport；资源计数为 0 时省略该段） */
function importResultText(report: BackupImportReport): string {
  const parts = [
    `新增 ${report.added}`,
    `更新 ${report.updated}`,
    `冲突 ${report.conflicts}`,
    `跳过 ${report.skipped}`,
  ]
  const assets = report.assetAdded + report.assetReused > 0
    ? ` · 资源新增 ${report.assetAdded} / 复用 ${report.assetReused}`
    : ''
  const hint = report.conflicts > 0 ? '（冲突已另存「导入冲突」副本）' : ''
  return `导入完成：${parts.join(' · ')}${assets}${hint}`
}

interface ImportPreviewDialogProps {
  fileName: string
  report: BackupImportReport
  applying: boolean
  error: string | null
  onCancel: () => void
  onConfirm: () => void
}

/**
 * 导入预览对话框（D4）：dry-run 结果只读展示，取消/关闭不写盘，只有「确认导入」才 apply。
 * portal / 遮罩 / Esc / 焦点圈定交给 ui/dialog；导入进行中屏蔽关闭与重复提交，避免半途关窗。
 */
function ImportPreviewDialog({ fileName, report, applying, error, onCancel, onConfirm }: ImportPreviewDialogProps) {
  const titles = report.conflictTitles ?? []

  return (
    <Dialog open onOpenChange={(open) => { if (!open && !applying) onCancel() }}>
      <DialogContent
        className="backup-dialog"
        overlayClassName="backup-dialog-overlay"
        aria-label="导入备份预览"
        onEscapeKeyDown={(event) => { if (applying) event.preventDefault() }}
        onPointerDownOutside={(event) => { if (applying) event.preventDefault() }}
      >
        <DialogTitle className="backup-dialog-title">导入备份预览</DialogTitle>
        <DialogDescription asChild>
          <div className="backup-dialog-file tiny text-muted" title={fileName}>{fileName}</div>
        </DialogDescription>

        <div className="backup-preview-counts" role="group" aria-label="导入变更统计">
          <span className="backup-preview-stat"><b>{report.added}</b><span>新增</span></span>
          <span className="backup-preview-stat"><b>{report.updated}</b><span>更新</span></span>
          <span className={`backup-preview-stat${report.conflicts > 0 ? ' is-danger' : ''}`}>
            <b>{report.conflicts}</b><span>冲突</span>
          </span>
          <span className="backup-preview-stat"><b>{report.skipped}</b><span>跳过</span></span>
        </div>
        <div className="backup-preview-assets tiny text-muted">
          资源：新增 {report.assetAdded} · 复用 {report.assetReused}
          {report.conflicts > 0 && '（冲突保留本地，备份版本另存「（导入冲突）」副本）'}
        </div>

        {titles.length > 0 && (
          <div className="backup-preview-conflicts">
            <div className="tiny text-muted">冲突标题</div>
            <ul className="backup-conflict-list">
              {titles.slice(0, CONFLICT_TITLE_PREVIEW).map((title, index) => (
                <li key={`${index}-${title}`} title={title}>{title}</li>
              ))}
            </ul>
            {titles.length > CONFLICT_TITLE_PREVIEW && (
              <div className="tiny text-muted">等 {titles.length} 条</div>
            )}
          </div>
        )}

        {error && <div className="inline-err" role="alert">{error}</div>}

        <div className="backup-dialog-actions">
          <Button variant="ghost" size="xs" type="button" disabled={applying} onClick={onCancel}>取消</Button>
          <Button
            variant="primary" size="xs"
            type="button"
            disabled={applying}
            aria-busy={applying}
            onClick={onConfirm}
          >
            {applying ? '导入中…' : '确认导入'}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
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
  const [kbCombo, setKbCombo] = useState(s.kbHotkey ?? '')
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

  /** 阅读字体（reading-font）：本地上传的字体清单 + 局部错误（不占用顶部整页错误条） */
  const [fonts, setFonts] = useState<ReadingFontInfo[]>([])
  const [fontBusy, setFontBusy] = useState(false)
  const [fontErr, setFontErr] = useState<string | null>(null)
  const fontFile = useRef<HTMLInputElement | null>(null)

  /** 阅读主题（kb-theme-library 3.1）：已导入清单 + 当前主题体检（识别/忽略/错误） */
  const [themes, setThemes] = useState<ReadingThemeInfo[]>([])
  const [themeBusy, setThemeBusy] = useState(false)
  const [themeErr, setThemeErr] = useState<string | null>(null)
  const [themeCheck, setThemeCheck] = useState<{
    ok: boolean
    recognized: number
    ignored: number
    errors: number
    reason?: string
  } | null>(null)
  const themeFile = useRef<HTMLInputElement | null>(null)

  /** 手动备份（kb-storage-backup 4.4）：导出结果 + 导入 dry-run 预览 / apply 状态，局部错误就近显示 */
  const [exporting, setExporting] = useState(false)
  const [exportError, setExportError] = useState<string | null>(null)
  const [exportResult, setExportResult] = useState<BackupExportResult | null>(null)
  const [importBusy, setImportBusy] = useState(false)
  const [importError, setImportError] = useState<string | null>(null)
  const [importNote, setImportNote] = useState<string | null>(null)
  /** 预览态：fileName 只用于展示，payload 与 dry-run 报告保活到确认/取消 */
  const [preview, setPreview] = useState<{
    fileName: string
    payload: string
    report: BackupImportReport
  } | null>(null)
  const [applyingImport, setApplyingImport] = useState(false)
  const [applyError, setApplyError] = useState<string | null>(null)
  const backupFile = useRef<HTMLInputElement | null>(null)

  // 可见 / 隐藏计数（批次 3：上限改为「可见便签合计 5」，不再按自由/知识分池）
  const visibleCount = stickyList.filter((s) => isStickyVisible(s)).length
  const hiddenCount = stickyList.length - visibleCount

  useEffect(() => {
    void getHotkeyStatus().then((r) => { if (!r.err && r.data) setHotkeys(r.data) })
    void loadStickies()
    void loadBackups()
    void loadFonts()
    void loadThemes()
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

  // 当前主题体检（3.1）：内置读打包资源、`file:` 走后端；读不到或 0 条有效声明 →「主题不可用」
  useEffect(() => {
    let cancelled = false
    const run = async () => {
      const value = s.readingTheme
      if (!value) { setThemeCheck(null); return }
      let css = ''
      if (value.startsWith('builtin:')) {
        css = BUILTIN_THEME_CSS[value.slice('builtin:'.length)] ?? ''
      } else if (value.startsWith('file:')) {
        const r = await readReadingTheme(value.slice('file:'.length))
        if (cancelled) return
        if (r.err || !r.data) {
          setThemeCheck({ ok: false, recognized: 0, ignored: 0, errors: 1, reason: '主题文件读不到' })
          return
        }
        css = r.data
      }
      if (cancelled) return
      if (!css) {
        setThemeCheck({ ok: false, recognized: 0, ignored: 0, errors: 1, reason: '主题不可用' })
        return
      }
      const parsed = parseTheme(css)
      const recognized = Object.keys(parsed.light).length + Object.keys(parsed.dark).length
      setThemeCheck({
        ok: parsed.ok && recognized > 0,
        recognized,
        ignored: parsed.ignored,
        errors: parsed.errors.length,
        reason: parsed.ok ? undefined : (parsed.errors[0] ?? '主题不可用'),
      })
    }
    void run()
    return () => { cancelled = true }
  }, [s.readingTheme])

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

  /** 已上传字体清单（<数据目录>/fonts/）；失败只污染本区块，不打断整页 */
  const loadFonts = async () => {
    const r = await listReadingFonts()
    if (r.err) { setFontErr(r.err); return }
    setFontErr(null)
    setFonts(r.data ?? [])
  }

  /** 已导入主题清单（<数据目录>/themes/）；失败只污染本区块 */
  const loadThemes = async () => {
    const r = await listReadingThemes()
    if (r.err) { setThemeErr(r.err); return }
    setThemeErr(null)
    setThemes(r.data ?? [])
  }

  /** 选字族：'' 跟随正文 / 'system:<键>' / 'file:<名>'；落库后 refresh → App 的 applyReadingFont 生效 */
  const chooseFont = async (value: string) => {
    await patch({ readingFont: value })
  }

  /** 上传：FileReader 读成 base64 交给后端（字节不过 JSON 数字数组），成功即启用 */
  const uploadFont = async (file: File) => {
    if (file.size > 16 * 1024 * 1024) {
      setFontErr('字体文件超过 16 MB，请换一个更小的')
      return
    }
    setFontBusy(true)
    setFontErr(null)
    try {
      const dataUrl = await new Promise<string>((resolve, reject) => {
        const reader = new FileReader()
        reader.onload = () => resolve(String(reader.result ?? ''))
        reader.onerror = () => reject(reader.error ?? new Error('读取失败'))
        reader.readAsDataURL(file)
      })
      const base64 = dataUrl.slice(dataUrl.indexOf(',') + 1)
      const r = await saveReadingFont(file.name, base64)
      if (r.err) { setFontErr(r.err); return }
      setFonts(r.data ?? [])
      await patch({ readingFont: `file:${file.name}` })
      setNote(`已安装并启用「${file.name}」`)
    } catch (e) {
      setFontErr('读取字体文件失败，请重试')
      console.warn('[settings] 字体上传失败', e)
    } finally {
      setFontBusy(false)
    }
  }

  /** 删除上传字体；删的若正在用，立刻退回「跟随正文」，避免下次启动加载失败 */
  const removeFont = async (name: string) => {
    setFontBusy(true)
    const r = await deleteReadingFont(name)
    setFontBusy(false)
    if (r.err) { setFontErr(r.err); return }
    setFonts(r.data ?? [])
    if (s.readingFont === `file:${name}`) await patch({ readingFont: '' })
  }

  /** 选主题：'' 默认外观 / 'builtin:<id>' / 'file:<名>' */
  const chooseTheme = async (value: string) => {
    await patch({ readingTheme: value })
  }

  /** 导入主题：明文 CSS 交给后端（≤512 KB、tmp+rename）；成功即启用 */
  const uploadTheme = async (file: File) => {
    if (file.size > 512 * 1024) {
      setThemeErr('主题文件超过 512 KB，请换一个更小的')
      return
    }
    setThemeBusy(true)
    setThemeErr(null)
    try {
      const text = await file.text()
      const r = await saveReadingTheme(file.name, text)
      if (r.err) { setThemeErr(r.err); return }
      setThemes(r.data ?? [])
      await patch({ readingTheme: `file:${file.name}` })
      setNote(`已导入并启用主题「${file.name}」`)
    } catch (e) {
      setThemeErr('读取主题文件失败，请重试')
      console.warn('[settings] 主题导入失败', e)
    } finally {
      setThemeBusy(false)
    }
  }

  /** 删除导入的主题；删的若正在用，立刻退回默认外观 */
  const removeTheme = async (name: string) => {
    setThemeBusy(true)
    const r = await deleteReadingTheme(name)
    setThemeBusy(false)
    if (r.err) { setThemeErr(r.err); return }
    setThemes(r.data ?? [])
    if (s.readingTheme === `file:${name}`) await patch({ readingTheme: '' })
  }

  const rebind = async (slot: 'capture' | 'main' | 'sticky' | 'todo' | 'kb') => {
    // 第 5 槽「直达知识库」走专用命令（D6）：失败保留旧绑定，成功返回最新注册快照
    if (slot === 'kb') {
      const r = await registerKbHotkey(kbCombo.trim() || null)
      if (r.err) { setErr(r.err); return }
      setErr(null)
      const st = await getHotkeyStatus()
      if (!st.err && st.data) {
        setHotkeys(st.data)
        const bound = st.data.kb ?? null
        setNote(bound ? `直达知识库快捷键：${bound}` : '直达知识库快捷键已清空（仍可从侧栏进入知识库）')
      } else {
        setNote('直达知识库快捷键已更新（新组合暂未生效，稍后可重试）')
      }
      await refresh()
      return
    }
    const r = slot === 'capture'
      ? await setSettings({ captureHotkey: capCombo.trim() })
      : slot === 'main'
        ? await setSettings({ mainHotkey: mainCombo.trim() || null })
        : slot === 'sticky'
          ? await setSettings({ stickyHotkey: stickyCombo.trim() || null })
          : await setSettings({ todoHotkey: todoCombo.trim() || null })
    if (r.err) { setErr(r.err); return }
    setErr(null)
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
    // 前四槽走 set_settings 事务；第 5 槽「直达知识库」走专用命令（D6），再统一回读注册快照
    const r = await setSettings({
      captureHotkey: DEFAULT_CAPTURE,
      mainHotkey: DEFAULT_MAIN,
      stickyHotkey: DEFAULT_STICKY,
      todoHotkey: DEFAULT_TODO,
    })
    if (r.err) { setErr(r.err); return }
    const kb = await registerKbHotkey(DEFAULT_KB)
    if (kb.err) { setErr(kb.err); return }
    setErr(null)
    setCapCombo(DEFAULT_CAPTURE)
    setMainCombo(DEFAULT_MAIN)
    setStickyCombo(DEFAULT_STICKY)
    setTodoCombo(DEFAULT_TODO)
    setKbCombo(DEFAULT_KB)
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

  /**
   * 导出单文件备份（D3）：命令内部已 spawn_blocking，前端只负责禁用与结果提示；
   * 成功后「打开数据文件夹」复用 open_data_folder（导出文件在 <数据目录>/backups/ 下）。
   */
  const doExportBackup = async () => {
    if (exporting) return
    setExporting(true)
    setExportError(null)
    setExportResult(null)
    const r = await exportBackup()
    setExporting(false)
    if (r.err) { setExportError(r.err); return }
    if (r.data) setExportResult(r.data)
  }

  /**
   * 选文件 → 读全文 → dry-run 预览（D4）：预览阶段绝不写盘；
   * 超过 256MB 直接拒绝（后端另有格式/大小校验，中文错误原样展示）。
   */
  const pickBackupFile = async (file: File) => {
    setImportError(null)
    setImportNote(null)
    if (file.size > BACKUP_FILE_MAX_BYTES) {
      setImportError('备份文件超过 256MB，无法导入')
      return
    }
    setImportBusy(true)
    try {
      const text = await file.text()
      const r = await importBackup(text, true)
      if (r.err) { setImportError(r.err); return }
      if (!r.data) { setImportError('备份预览失败，请重试'); return }
      setApplyError(null)
      setPreview({ fileName: file.name, payload: text, report: r.data })
    } catch (e) {
      console.warn('[settings] 读取备份文件失败', e)
      setImportError('读取备份文件失败，请重试')
    } finally {
      setImportBusy(false)
    }
  }

  /** 取消预览：只关弹窗，dry-run 阶段没有任何写盘（D4） */
  const cancelImportPreview = () => {
    if (applyingImport) return
    setPreview(null)
    setApplyError(null)
  }

  /**
   * 确认导入：复用预览时的 payload 走 apply（false），成功后显式 refresh 一次；
   * 后端 apply 后还会 emit store-changed（App.tsx 已有监听，既有全局链路），这里不重复订阅/刷新。
   * apply 前会轮转 data.json / notes.json，所以顺带重拉两份备份列表。
   */
  const applyImportPreview = async () => {
    if (!preview || applyingImport) return
    setApplyingImport(true)
    setApplyError(null)
    const r = await importBackup(preview.payload, false)
    setApplyingImport(false)
    if (r.err) { setApplyError(r.err); return }
    setPreview(null)
    setImportNote(r.data ? importResultText(r.data) : '导入完成')
    await refresh()
    void loadBackups()
    void loadKbBackups()
  }

  return (
    <div className="view">
      <div>
        <div className="view-title">设置</div>
        <div className="view-sub">改动即存 · 改错了右键「恢复默认」即可</div>
      </div>

      {err && <div className="inline-err" role="alert">{err}</div>}
      {note && !err && <div className="inline-note" role="status">{note}</div>}

      <section className="section" aria-label="外观">
        <div className="section-head"><b>外观</b></div>
        <div className="set-row"><span>主题</span>
          <div className="seg" role="group" aria-label="主题">
            <Button size="sm" aria-pressed={s.theme !== 'dark'} onClick={() => { void patch({ theme: 'light' }) }}>纸白</Button>
            <Button size="sm" aria-pressed={s.theme === 'dark'} onClick={() => { void patch({ theme: 'dark' }) }}>暗色</Button>
          </div>
        </div>
        <div className="set-row"><span>便签纸色
          <div className="tiny text-muted">便签规格 §12.1：预设四选一，分类色不随纸色变</div>
        </span>
          <div className="seg" role="group" aria-label="便签纸色">
            {STICKY_PAPERS.map((p) => (
              <Button
                key={p.key}
                size="sm" aria-pressed={s.stickyPaper === p.key}
                onClick={() => { void patch({ stickyPaper: p.key }) }}
              >
                <span className={`paper-dot paper-${p.key}`} aria-hidden />
                {p.label}
              </Button>
            ))}
          </div>
        </div>
      </section>

      <section className="section" aria-label="阅读字体">
        <div className="section-head">
          <b>阅读字体</b>
          <span>只作用于知识库正文 · 报头衬线与界面文字不跟随</span>
        </div>
        {fontErr && <div className="inline-err" role="alert">{fontErr}</div>}
        <div className="set-row"><span>字族
          <div className="tiny text-muted">系统字体调用本机已装字体；上传的字体存进数据目录 fonts/，只留在这台机器</div>
        </span>
          <div className="seg" role="group" aria-label="阅读字体">
            <Button size="sm" aria-pressed={!s.readingFont} onClick={() => { void chooseFont('') }}>跟随正文</Button>
            {READING_FONT_SYSTEMS.map((f) => (
              <Button
                key={f.key}
                size="sm" aria-pressed={s.readingFont === `system:${f.key}`}
                onClick={() => { void chooseFont(`system:${f.key}`) }}
              >
                {f.label}
              </Button>
            ))}
          </div>
        </div>
        <div className="set-row"><span>排版档位
          <div className="tiny text-muted">字号 / 字距 / 列宽已移到文档工具条的「排版」弹层里调</div>
        </span>
          <span className="tiny text-muted">打开任一知识条目 → 顶部工具条右侧「A」</span>
        </div>
        <div className="set-row"><span>上传字体
          <div className="tiny text-muted">支持 ttf / otf / woff / woff2，单文件 ≤ 16 MB</div>
        </span>
          <span className="row-flex items-center gap-2">
            <input
              ref={fontFile}
              type="file"
              accept=".ttf,.otf,.woff,.woff2"
              className="font-file"
              onChange={(e) => {
                const picked = e.target.files?.[0]
                if (picked) void uploadFont(picked)
                e.target.value = ''
              }}
            />
            <Button size="sm" disabled={fontBusy} onClick={() => fontFile.current?.click()}>
              {fontBusy ? '处理中…' : '选择文件…'}
            </Button>
          </span>
        </div>
        {fonts.length > 0 && (
          <div className="set-row"><span>已安装
            <div className="tiny text-muted">删除正在使用的字体会自动退回「跟随正文」</div>
          </span>
            <div className="font-list">
              {fonts.map((f) => (
                <span key={f.name} className={`font-chip ${s.readingFont === `file:${f.name}` ? 'on' : ''}`}>
                  <Button
                    variant="bare"
                    className="font-chip-name"
                    title={f.name}
                    onClick={() => { void chooseFont(`file:${f.name}`) }}
                  >
                    {f.name}
                  </Button>
                  <span className="font-chip-size mono">{f.sizeKb} KB</span>
                  <Button
                    variant="bare"
                    className="font-chip-del"
                    aria-label={`删除 ${f.name}`}
                    disabled={fontBusy}
                    onClick={() => { void removeFont(f.name) }}
                  >
                    ×
                  </Button>
                </span>
              ))}
            </div>
          </div>
        )}
        <div className="set-row"><span>预览</span>
          <p className="font-preview">纸上得来终觉浅，绝知此事要躬行 · 0123456789 · The quick brown fox</p>
        </div>
      </section>

      <section className="section" aria-label="外观主题">
        <div className="section-head">
          <b>外观主题</b>
          <span>作用于整个应用 · 亮暗各一套值 · 不改变字号、字距与列宽</span>
        </div>
        {themeErr && <div className="inline-err" role="alert">{themeErr}</div>}
        <div className="set-row"><span>主题
          <div className="tiny text-muted">主窗、快速记录、便签、待办悬浮一起换装；导入的 CSS 存进数据目录 themes/，只留在这台机器</div>
        </span>
          <div className="seg" role="group" aria-label="外观主题">
            <Button size="sm" aria-pressed={!s.readingTheme} onClick={() => { void chooseTheme('') }}>默认</Button>
            {Object.entries(BUILTIN_THEME_LABELS).map(([id, label]) => (
              <Button
                key={id}
                size="sm"
                aria-pressed={s.readingTheme === `builtin:${id}`}
                onClick={() => { void chooseTheme(`builtin:${id}`) }}
              >
                {label}
              </Button>
            ))}
          </div>
        </div>
        <div className="set-row"><span>导入主题
          <div className="tiny text-muted">单个 CSS 文件，≤ 512 KB；只认文件里的 kettd-theme 声明与 .kb-theme-light / .kb-theme-dark 两块</div>
        </span>
          <span className="row-flex items-center gap-2">
            <input
              ref={themeFile}
              type="file"
              accept=".css"
              className="font-file"
              onChange={(e) => {
                const picked = e.target.files?.[0]
                if (picked) void uploadTheme(picked)
                e.target.value = ''
              }}
            />
            <Button size="sm" disabled={themeBusy} onClick={() => themeFile.current?.click()}>
              {themeBusy ? '处理中…' : '选择文件…'}
            </Button>
          </span>
        </div>
        {themes.length > 0 && (
          <div className="set-row"><span>已导入
            <div className="tiny text-muted">删除正在使用的主题会立刻退回默认外观</div>
          </span>
            <div className="font-list">
              {themes.map((t) => (
                <span key={t.name} className={`font-chip ${s.readingTheme === `file:${t.name}` ? 'on' : ''}`}>
                  <Button
                    variant="bare"
                    className="font-chip-name"
                    title={t.name}
                    onClick={() => { void chooseTheme(`file:${t.name}`) }}
                  >
                    {t.name}
                  </Button>
                  <span className="font-chip-size mono">{t.sizeKb} KB</span>
                  <Button
                    variant="bare"
                    className="font-chip-del"
                    aria-label={`删除 ${t.name}`}
                    disabled={themeBusy}
                    onClick={() => { void removeTheme(t.name) }}
                  >
                    ×
                  </Button>
                </span>
              ))}
            </div>
          </div>
        )}
        {s.readingTheme && (
          <div className="set-row"><span>主题体检</span>
            {themeCheck && !themeCheck.ok ? (
              <span className="tiny" role="status">
                主题不可用：{themeCheck.reason ?? '没有可用的声明'}（设置值保留，修好后自动恢复）
              </span>
            ) : (
              <span className="tiny text-muted" role="status">
                {themeCheck
                  ? `识别 ${themeCheck.recognized} 条 / 忽略 ${themeCheck.ignored} 条 / 错误 ${themeCheck.errors} 条`
                  : '检查中…'}
              </span>
            )}
          </div>
        )}
      </section>

      <section className="section" aria-label="便签">
        <div className="section-head"><b>便签</b><span>随手一张纸 · 常驻置顶 · 关闭后保留，可在下方重新打开</span></div>
        {stickyError && <div className="inline-err" role="alert">{stickyError}</div>}
        <div className="set-row"><span>纸面花纹
          <div className="tiny text-muted">水印级纹样，选墨竹/远山时折角旁伴一枚朱印</div>
        </span>
          <div className="seg" role="group" aria-label="纸面花纹">
            {[{ key: 'none', label: '无' }, { key: 'bamboo', label: '墨竹' }, { key: 'mountain', label: '远山' }].map((p) => (
              <Button
                key={p.key}
                size="sm" aria-pressed={s.stickyPattern === p.key}
                onClick={() => { void patch({ stickyPattern: p.key }) }}
              >
                {p.label}
              </Button>
            ))}
          </div>
        </div>
        <div className="set-row"><span>移出淡化
          <div className="tiny text-muted">鼠标移出便签时淡至下方透明度，融入桌面仍可扫读（缩小条不淡化）</div>
        </span>
          <span className="row-flex items-center gap-2">
            <div className="seg" role="group" aria-label="移出淡化">
              <Button size="sm" aria-pressed={s.stickyFade} onClick={() => { void patch({ stickyFade: true }) }}>开</Button>
              <Button size="sm" aria-pressed={!s.stickyFade} onClick={() => { void patch({ stickyFade: false }) }}>关</Button>
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
            <Button size="xs" onClick={() => { void doCreateSticky() }}>新建便签</Button>
          </span>
        </div>
        <div className="set-row"><span>管理便签
          <div className="tiny text-muted">卡片墙：搜索正文与知识标题，打开、删除一屏完成</div>
        </span>
          <Button size="xs" onClick={() => { window.location.hash = '#/sticky' }}>管理便签</Button>
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
                    <Button
                      variant="ghost" size="xs"
                      onClick={() => { void doOpenSticky(st) }}
                      aria-label={`打开便签：${preview}`}
                    >
                      打开
                    </Button>
                    <Button variant="ghost" size="xs" onClick={() => { void doToggleSticky(st) }}>{st.mini ? '展开' : '缩小'}</Button>
                    {confirmDel === st.id ? (
                      <span className="row-flex items-center gap-1">
                        <Button
                          variant="danger"
                          size="xs"
                          onClick={() => { void doDeleteSticky(st) }}
                          title="删除后不可恢复；关闭便签只是隐藏"
                        >
                          确认删除
                        </Button>
                        <Button variant="ghost" size="xs" onClick={() => setConfirmDel(null)}>取消</Button>
                      </span>
                    ) : (
                      <Button
                        variant="ghost" size="xs"
                        onClick={() => setConfirmDel(st.id)}
                        title="删除后不可恢复；关闭便签只是隐藏"
                      >
                        删除
                      </Button>
                    )}
                  </span>
                )
              })}
            </span>
          </div>
        )}
      </section>

      <section className="section" aria-label="快捷键">
        <div className="section-head"><b>快捷键</b><span>{hotkeys ? '以下是当前真正生效的组合' : '读取中…'}</span></div>
        <div className="set-row"><span>快速记录
          <div className="tiny text-muted">
            当前绑定：{hotkeys?.capture ?? '未生效'}
            {hotkeys && !hotkeys.capture && '（被占用或未注册，可改后重试）'}
          </div>
        </span>
          <span className="row-flex">
            <Input size="xs" value={capCombo} onChange={(e) => setCapCombo(e.target.value)} aria-label="快速记录热键" />
            <Button size="xs" onClick={() => { void rebind('capture') }}>换绑</Button>
          </span>
        </div>
        <div className="set-row"><span>打开主界面
          <div className="tiny text-muted">当前绑定：{hotkeys?.main ?? '未绑定'}（留空 = 解绑）</div>
        </span>
          <span className="row-flex">
            <Input size="xs" value={mainCombo} onChange={(e) => setMainCombo(e.target.value)} aria-label="主界面热键" />
            <Button size="xs" onClick={() => { void rebind('main') }}>换绑</Button>
          </span>
        </div>
        <div className="set-row"><span>新建便签
          <div className="tiny text-muted">
            随手新建一张便签（不管主窗开没开）。当前绑定：{hotkeys?.sticky ?? '未绑定'}（留空 = 解绑）
          </div>
        </span>
          <span className="row-flex">
            <Input size="xs" value={stickyCombo} onChange={(e) => setStickyCombo(e.target.value)} aria-label="新建便签热键" />
            <Button size="xs" onClick={() => { void rebind('sticky') }}>换绑</Button>
          </span>
        </div>
        <div className="set-row"><span>待办悬浮窗
          <div className="tiny text-muted">
            呼出/隐藏今天待办的悬浮窗。当前绑定：{hotkeys?.todo ?? '未绑定'}（留空 = 解绑）
          </div>
        </span>
          <span className="row-flex">
            <Input size="xs" value={todoCombo} onChange={(e) => setTodoCombo(e.target.value)} aria-label="待办悬浮窗热键" />
            <Button size="xs" onClick={() => { void rebind('todo') }}>换绑</Button>
          </span>
        </div>
        <div className="set-row"><span>直达知识库
          <div className="tiny text-muted">
            隐藏主窗时按一下：唤出主界面并停在知识库。当前绑定：{hotkeys?.kb ?? '未绑定'}（留空 = 解绑）
          </div>
        </span>
          <span className="row-flex">
            <Input size="xs" value={kbCombo} onChange={(e) => setKbCombo(e.target.value)} aria-label="直达知识库热键" />
            <Button size="xs" onClick={() => { void rebind('kb') }}>换绑</Button>
          </span>
        </div>
        <div className="set-row"><span>恢复默认
          <div className="tiny text-muted">快速记录 {DEFAULT_CAPTURE} · 打开主界面 {DEFAULT_MAIN} · 新建便签 {DEFAULT_STICKY} · 待办悬浮窗 {DEFAULT_TODO} · 直达知识库 {DEFAULT_KB}</div>
        </span>
          <Button size="xs" onClick={() => { void restoreDefaults() }}>恢复默认快捷键</Button>
        </div>
      </section>

      <section className="section" aria-label="提醒">
        <div className="section-head"><b>提醒</b></div>
        <div className="set-row"><span>每小时上限
          <div className="tiny text-muted">超出排队等下个窗口，绝不轰炸</div>
        </span>
          <div className="seg" role="group" aria-label="每小时提醒上限">
            {[3, 5, 10].map((n) => (
              <Button
                key={n}
                size="sm" aria-pressed={s.remindCapPerHour === n}
                onClick={() => { void patch({ remindCapPerHour: n }) }}
              >
                {n} 条
              </Button>
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
          <Button size="xs" onClick={() => { void openDataFolder() }}>打开数据文件夹</Button>
        </div>
        <div className="set-row"><span>本地度量
          <div className="tiny text-muted">仅本机统计，绝不出网 · 用于验证功能是否真被用上</div>
        </span>
          <span className="row-flex items-center gap-1.5">
            <div className="seg" role="group" aria-label="本地度量">
              <Button size="sm" aria-pressed={s.telemetryEnabled} onClick={() => { void patch({ telemetryEnabled: true }) }}>开</Button>
              <Button size="sm" aria-pressed={!s.telemetryEnabled} onClick={() => { void patch({ telemetryEnabled: false }) }}>关</Button>
            </div>
            <Button
              variant={confirmClear ? 'danger' : 'default'} size="xs"
              aria-label={confirmClear ? '再次点击确认清空本地统计' : '清空本地统计'}
              onClick={() => { void doClearEvents() }}
            >
              {confirmClear ? '确认清空？' : '清空本地统计'}
            </Button>
          </span>
        </div>
      </section>

      <section className="section" aria-label="备份与恢复">
        <div className="section-head">
          <b>备份与恢复</b>
          <span>每次保存自动留存 5 份 · 恢复会用选中备份覆盖当前数据，操作不可撤销</span>
        </div>

        <div className="set-row" style={{ alignItems: 'flex-start' }}>
          <span>任务数据备份
            <div className="tiny text-muted">
              每次保存自动留存 5 份历史（data.json.bak-1 … bak-5），恢复前也会先留存、不会挤掉所选那份
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
                      <Button
                        variant={confirming ? 'danger' : 'default'} size="xs"
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
                      </Button>
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
              每次保存知识库自动留存 5 份历史（notes.json.bak-1 … bak-5），恢复后覆盖当前知识库
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
                      <Button
                        variant={confirming ? 'danger' : 'default'} size="xs"
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
                      </Button>
                    </span>
                  )
                })}
              </span>
            )}
          </span>
        </div>
      </section>

      <section className="section" aria-label="手动备份">
        <div className="section-head">
          <b>手动备份</b>
          <span>单文件 .kettdbackup · 含知识正文与引用资源 · 导入先预览、确认后增量合并，绝不删除本地条目</span>
        </div>

        <div className="set-row manual-backup-row">
          <span>导出备份
            <div className="tiny text-muted">
              导出到数据目录的 backups/ 里（文件名带导出时间）；不动当前数据，也不会顶掉已有的历史备份
            </div>
          </span>
          <span className="backup-list">
            <span className="row-flex items-center gap-1.5">
              <Button
                size="xs"
                disabled={exporting}
                aria-busy={exporting}
                onClick={() => { void doExportBackup() }}
              >
                {exporting ? '导出中…' : '导出备份'}
              </Button>
              {exportResult && (
                <Button variant="ghost" size="xs" onClick={() => { void openDataFolder() }}>打开数据文件夹</Button>
              )}
            </span>
            {exportError && <span className="inline-err" role="alert">{exportError}</span>}
            {exportResult && !exportError && (
              <span className="inline-note backup-manual-note" role="status">
                已导出到 {exportResult.path}（{exportResult.itemCount} 条知识，{exportResult.assetCount} 个资源，{exportResult.skipped} 项跳过）
              </span>
            )}
          </span>
        </div>

        <div className="set-row manual-backup-row">
          <span>导入备份
            <div className="tiny text-muted">
              先预览会比当前多出多少、更新多少、哪几条两边都有，确认后才真正写入；导入前会先留存一份当前数据
            </div>
          </span>
          <span className="backup-list">
            <input
              ref={backupFile}
              className="backup-file-input"
              type="file"
              accept=".kettdbackup"
              aria-label="选择 .kettdbackup 备份文件"
              onChange={(event) => {
                const picked = event.target.files?.[0]
                // 清空 value：允许连续选择同一个文件（否则第二次 change 不触发）
                event.target.value = ''
                if (picked) void pickBackupFile(picked)
              }}
            />
            <span className="row-flex items-center gap-1.5">
              <Button
                size="xs"
                disabled={importBusy || preview !== null}
                aria-busy={importBusy}
                onClick={() => backupFile.current?.click()}
              >
                {importBusy ? '读取中…' : '选择备份文件'}
              </Button>
              <span className="tiny text-muted">仅 .kettdbackup，不超过 256MB</span>
            </span>
            {importError && <span className="inline-err" role="alert">{importError}</span>}
            {importNote && !importError && <span className="inline-note backup-manual-note" role="status">{importNote}</span>}
          </span>
        </div>
      </section>

      {preview && (
        <ImportPreviewDialog
          fileName={preview.fileName}
          report={preview.report}
          applying={applyingImport}
          error={applyError}
          onCancel={cancelImportPreview}
          onConfirm={() => { void applyImportPreview() }}
        />
      )}

      <div className="version-line text-muted">Kettd v{boot.version} · 离线保存</div>
    </div>
  )
}
