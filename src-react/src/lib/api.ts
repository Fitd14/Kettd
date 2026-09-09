/**
 * Kettd v3 · 类型化 Tauri 桥接（M2）。
 * 契约真相源：src-tauri/docs/V2-API.md —— 41 个命令逐一对应，参数名一律 camelCase；
 * 延续 vanilla 时代 { data, err } 约定（M4 后 vanilla 已退役）：err 为后端可直接展示的中文短句，永不 throw。
 * 事件补发队列语义住在 src/kernel/event-bus.js（E14 唯一实现，测试 test/event-bus.test.mjs）。
 * 时间全链路本地语义字符串，零 UTC 换算（不用 toISOString）。
 */
import { createEventBus } from '../kernel/event-bus.js'
import { localToday, localNowHHMM, addDays, fullDueParts, WEEK_CN } from '../kernel/time.js'

export { localToday, localNowHHMM, addDays }

/* ---------------------------------------------------------------- 领域类型（对齐 models.rs serde camelCase） */

export type Category = '工作' | '学习' | '生活'
export type Priority = 'high' | 'med' | 'low'

export interface Subtask {
  id: string
  title: string
  done: boolean
}

export interface Note {
  id: string
  author: string
  content: string
  createdAt: string
}

export interface Task {
  id: string
  title: string
  note: string
  category: string
  priority: string
  dueAt?: string | null
  remindAt?: string | null
  plannedDate?: string | null
  carriedFrom: number
  done: boolean
  doneAt?: string | null
  deletedAt?: string | null
  legacy?: boolean
  /** 单向 task→KB 引用（frame H1b 预埋） */
  kbRefs?: string[]
  /** 同列表手动排序位（便签规格 §12.2）；缺省 = 未手动排过 */
  sortOrder?: number | null
  subtasks: Subtask[]
  notes: Note[]
  source: 'capture' | 'manual' | 'seed'
  createdAt: string
  updatedAt: string
}

export interface TaskPayload {
  title?: string
  note?: string | null
  category?: string | null
  priority?: string | null
  dueAt?: unknown
  remindAt?: unknown
  plannedDate?: unknown
  carriedFrom?: number
  done?: boolean
  doneAt?: unknown
  deletedAt?: unknown
  legacy?: boolean
  kbRefs?: string[] | null
  subtasks?: Subtask[]
  notes?: Note[]
  source?: Task['source']
  createdAt?: string
}

export interface Reminder {
  id: string
  title: string
  /** `HH:MM` / `HH:MM/HH:MM`（循环）或 `YYYY-MM-DDTHH:mm`（单次） */
  time: string
  category: string
  enabled: boolean
  completed?: boolean
  repeat: string
  lastFired?: string | null
  snoozedUntil?: string | null
  legacy?: boolean
}

export interface ReminderPayload {
  title?: string
  time?: string
  category?: string
  enabled?: boolean
  completed?: boolean
  repeat?: string
  snoozedUntil?: string | null
}

export interface KbItem {
  id: string
  title: string
  bodyMd: string
  tags: string[]
  createdAt: string
  updatedAt: string
}

export interface KbPayload {
  title?: string
  bodyMd?: string
  tags?: string[]
}

export interface Dnd {
  enabled: boolean
  from: string
  to: string
}

export interface Settings {
  theme: string
  stickyPaper: string
  /** 移出淡化开关（planned-settings B.1★，默认开） */
  stickyFade: boolean
  /** 移出淡化后的不透明度（10-100，默认 38） */
  stickyFadeOpacity: number
  /** 纸面花纹：none 无 | bamboo 墨竹 | mountain 远山（选后两者时朱印伴随） */
  stickyPattern: string
  captureHotkey: string
  mainHotkey: string | null
  /** 新建便签全局热键（sticky-separation）；null = 已解绑 */
  stickyHotkey: string | null
  /** 待办悬浮窗置顶（todo-float，默认开；窗内 Pin 钮同源） */
  todoFloatPinned: boolean
  /** 待办悬浮窗呼出/隐藏热键；null = 已解绑 */
  todoHotkey: string | null
  dnd: Dnd
  remindCapPerHour: number
  onboarded: boolean
  exportDir?: string | null
  telemetryEnabled: boolean
}

export interface SettingsPayload {
  theme?: string
  stickyPaper?: string
  stickyFade?: boolean
  stickyFadeOpacity?: number
  stickyPattern?: string
  captureHotkey?: string
  mainHotkey?: string | null
  stickyHotkey?: string | null
  todoFloatPinned?: boolean
  todoHotkey?: string | null
  dnd?: Partial<Dnd>
  remindCapPerHour?: number
  onboarded?: boolean
  exportDir?: string | null
  telemetryEnabled?: boolean
}

export interface BackupInfo {
  slot: string
  name: string
  createdAt: string
  sizeKb: number
  taskCount: number
  readable: boolean
  error?: string | null
}

export interface DataHealthV2 {
  health: string
  backups: BackupInfo[]
  corruptFile?: string | null
  lastError?: string | null
  writable: boolean
}

export interface MigrationIssue {
  field?: string
  message?: string
  [key: string]: unknown
}

export interface MigrationReport {
  taskCount: number
  reminderCount: number
  invalidDates: number
  legacyDone: number
  archivedTo: string
  issues: MigrationIssue[]
}

export interface Bootstrap {
  tasks: Task[]
  reminders: Reminder[]
  settings: Settings
  health: DataHealthV2
  migration?: MigrationReport | null
  /** 应用版本（设置页展示，Cargo.toml 为真相源） */
  version: string
}

export interface HotkeyStatus {
  capture: string | null
  main: string | null
  sticky: string | null
  todo: string | null
}

export interface RestoreResult {
  restored: number
  health: DataHealthV2
}

export interface StoreEvent {
  id: string
  kind: 'changed' | 'fired' | 'missed'
}

export type CallResult<T> = { data: T | null; err: string | null }

/* ---------------------------------------------------------------- 桥接原语 */

type TauriWindowApi = {
  minimize?: () => Promise<void>
  toggleMaximize?: () => Promise<void>
  close?: () => Promise<void>
  isMaximized?: () => Promise<boolean>
  onResized?: (handler: (ev: unknown) => void) => Promise<() => void>
}

type TauriGlobal = {
  tauri?: { invoke?: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> }
  core?: { invoke?: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> }
  invoke?: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>
  event?: {
    listen?: (name: string, handler: (ev: unknown) => void) => Promise<() => void>
    emit?: (name: string, payload?: unknown) => Promise<void>
  }
  listen?: (name: string, handler: (ev: unknown) => void) => Promise<() => void>
  emit?: (name: string, payload?: unknown) => Promise<void>
  window?: { appWindow?: TauriWindowApi }
}

/** 调用时惰性取桥（v2.2 加固）：__TAURI__ 注入晚于模块求值也能接上，不再信任加载时序 */
function tauriBridge(): TauriGlobal {
  return (globalThis as { __TAURI__?: TauriGlobal }).__TAURI__ ?? {}
}

/** 当前窗的窗口操作桥（自定义标题栏方案B：最小化/最大化/关闭）；未连接桌面运行时返回 null */
export function mainWindowBridge(): TauriWindowApi | null {
  return tauriBridge().window?.appWindow ?? null
}

const rawListen = tauriBridge().event?.listen ?? tauriBridge().listen
const rawEmit = tauriBridge().event?.emit ?? tauriBridge().emit

async function call<T>(command: string, args?: Record<string, unknown>): Promise<CallResult<T>> {
  const T = tauriBridge()
  const rawInvoke = T.tauri?.invoke ?? T.core?.invoke ?? T.invoke
  if (typeof rawInvoke !== 'function') return { data: null, err: '未连接桌面运行时，命令未执行' }
  try {
    const data = await rawInvoke(command, args ?? {})
    return { data: ((data ?? null) as T), err: null }
  } catch (e) {
    const err = typeof e === 'string' ? e : ((e as { message?: string })?.message
      ?? (e as { payload?: string })?.payload) ?? String(e)
    return { data: null, err }
  }
}

/* ---------------------------------------------------------------- 事件总线（内核 E14 唯一实现） */

const bus = createEventBus({ listen: rawListen })
export const onEvent = bus.onEvent
export const eventOnce = bus.eventOnce

/* ---------------------------------------------------------------- 命令封装（41，与 V2-API.md §2 表逐一对应） */

export const getBootstrap = () => call<Bootstrap>('get_bootstrap')
export const getTasks = (includeDeleted?: boolean) => call<Task[]>('get_tasks', { includeDeleted: !!includeDeleted })
export const getTask = (id: string) => call<Task>('get_task', { id })
export const addTask = (args: TaskPayload) => call<Task>('add_task', { args })
export const updateTask = (id: string, patch: TaskPayload) => call<Task>('update_task', { id, patch })
/** 同列表手动排序（便签规格 §12.2）：整表传入目标顺序 */
export const reorderTasks = (idsInOrder: string[]) => call<number>('reorder_tasks', { idsInOrder })
export const toggleTask = (id: string) => call<Task>('toggle_task', { id })
export const deleteTask = (id: string) => call<Task>('delete_task', { id })
export const undoDelete = () => call<Task | null>('undo_delete')
export const restoreTask = (id: string) => call<Task>('restore_task', { id })
export const purgeTask = (id: string) => call<Task>('purge_task', { id })
export const addSubtask = (taskId: string, title: string) => call<Subtask>('add_subtask', { taskId, title })
export const toggleSubtask = (taskId: string, subtaskId: string) => call<Subtask>('toggle_subtask', { taskId, subtaskId })
export const deleteSubtask = (taskId: string, subtaskId: string) => call<Subtask>('delete_subtask', { taskId, subtaskId })
export const addNote = (taskId: string, note: { author?: string; content: string }) => call<Note>('add_note', { taskId, note })
export const deleteNote = (taskId: string, noteId: string) => call<Note>('delete_note', { taskId, noteId })
export const getReminders = () => call<Reminder[]>('get_reminders')
export const addReminder = (reminder: ReminderPayload) => call<Reminder>('add_reminder', { reminder })
export const updateReminder = (id: string, patch: ReminderPayload) => call<Reminder>('update_reminder', { id, patch })
export const deleteReminder = (id: string) => call<void>('delete_reminder', { id })
export const toggleReminder = (id: string) => call<Reminder>('toggle_reminder', { id })
export const snoozeReminder = (id: string, minutes: number) => call<Reminder>('snooze_reminder', { id, minutes })
export const getSettings = () => call<Settings>('get_settings')
export const setSettings = (patch: SettingsPayload) => call<Settings>('set_settings', { patch })
/** 清空本地统计（设置页二次确认后调用）：重置 events.jsonl */
export const clearEvents = () => call<void>('clear_events')
/** 通用前端埋点（metric 蓝图：weekly_open 等 UI 侧事件），仅落本机 events.jsonl */
export const trackEvent = (name: string, props?: Record<string, unknown>) =>
  call<void>('track_event', { name, props: props ? JSON.stringify(props) : null })

/* ---------------------------------------------------------------- 便签（sticky-separation：纯自由便签，多开） */

export interface StickyNoteItem {
  id: string
  content: string
  /** 缩小态：置顶悬浮文本条（显示正文第一行，点击展开） */
  mini: boolean
  /** Phase 2：知识便签视口指针（None = 自由便签） */
  kbRef?: string | null
  createdAt: string
  updatedAt: string
}

export interface StickySelf {
  id: string
  content: string
  mini: boolean
  /** Phase 2：知识便签视口指针（None = 自由便签） */
  kbRef?: string | null
}

export const createSticky = (args?: { kbRef?: string }) =>
  call<StickyNoteItem>('create_sticky', { args: args ?? {} })
export const listStickies = () => call<StickyNoteItem[]>('list_stickies')
export const updateSticky = (
  id: string,
  patch: { content?: string; mini?: boolean },
) =>
  call<StickyNoteItem>('update_sticky', {
    id,
    content: patch.content ?? null,
    mini: patch.mini ?? null,
  })
/** 关闭即销毁（一次性工具语义）：窗口与数据一并回收 */
export const deleteSticky = (id: string) => call<void>('delete_sticky', { id })
/** 便签窗启动时调一次：拿到自己是谁（label → id/内容/形态） */
export const stickySelf = () => call<StickySelf>('sticky_self')
export const openMainWindow = (route?: string) => call<void>('open_main_window', route ? { route } : undefined)
export const openDataFolder = () => call<void>('open_data_folder')
export const registerCaptureHotkey = (combo: string) => call<void>('register_capture_hotkey', { combo })
export const registerMainHotkey = (combo: string | null) => call<void>('register_main_hotkey', { combo })
/** 新建便签热键（默认 Alt+Shift+S，可解绑）；空串 = 解绑 */
export const registerStickyHotkey = (combo: string | null) => call<void>('register_sticky_hotkey', { combo })
/** 待办悬浮窗呼出/隐藏热键（默认 Alt+Shift+T，可解绑）；空串 = 解绑 */
export const registerTodoHotkey = (combo: string | null) => call<void>('register_todo_hotkey', { combo })
/** 隐藏待办悬浮窗（✕ 同款）：只藏窗，显隐状态落盘 */
export const hideTodoFloat = () => call<void>('hide_todo_float')
/** 热键实际注册快照（null = 未绑上/已解绑），设置页据此标「未生效」 */
export const getHotkeyStatus = () => call<HotkeyStatus>('get_hotkey_status')
export const openCaptureOverlay = () => call<void>('open_capture_overlay')
export const closeCaptureOverlay = () => call<void>('close_capture_overlay')
export const captureStartDrag = () => call<void>('capture_start_drag')
/** 内容高度上报：快速记录条窗口顶部锚定向下伸缩（方案B 动态高度） */
export const captureResize = (height: number) => call<void>('capture_resize', { height })
/** 已注册但前端未接线（V2-API §10）——补齐封装，接线随 M3 */
export const exportWeekly = (opts?: { week?: string; format?: 'md' | 'csv'; dir?: string }) => {
  const args: Record<string, unknown> = {}
  if (opts?.week != null) args.week = opts.week
  if (opts?.format != null) args.format = opts.format
  if (opts?.dir != null) args.dir = opts.dir
  return call<string>('export_weekly', args)
}
export const getBackups = () => call<BackupInfo[]>('get_backups')
export const restoreBackup = (slot: string) => call<RestoreResult>('restore_backup', { slot })
export const getDataHealth = () => call<DataHealthV2>('get_data_health')
export const clearMigrationReport = () => call<MigrationReport | null>('clear_migration_report')
export const getFormHints = () => call<{ examples: string[]; categories: string[]; priorities: string[] }>('get_form_hints')
/** 调试入口（不进 UI，ADR-0005） */
export const rollbackSchemaSplit = () => call<string>('rollback_schema_split')

/* 知识库（frame H1b：库 + 搜索 + 任务单向引用；链接/unlink 走 updateTask 的 kbRefs 补丁） */
export const getKbItems = () => call<KbItem[]>('get_kb_items')
export const addKbItem = (args: KbPayload) => call<KbItem>('add_kb_item', { args })
export const updateKbItem = (id: string, patch: KbPayload) => call<KbItem>('update_kb_item', { id, patch })
export const deleteKbItem = (id: string) => call<void>('delete_kb_item', { id })
export const searchKb = (query?: string) => call<KbItem[]>('search_kb', { query: query ?? null })

/* ---------------------------------------------------------------- 跨窗导航（E14 localStorage 兜底，原样保留） */

export function emitNavigate(route: string) {
  if (typeof rawEmit === 'function') {
    try {
      const p = rawEmit('navigate', route)
      if (p && typeof (p as Promise<void>).catch === 'function') {
        (p as Promise<void>).catch(() => persistRoute(route))
      }
      return
    } catch { /* fall through */ }
  }
  persistRoute(route)
}

function persistRoute(route: string) {
  try { localStorage.setItem('kettd.pending-route', String(route)) } catch { /* 隔离环境忽略 */ }
}

export function takePendingRoute(): string | null {
  try {
    const v = localStorage.getItem('kettd.pending-route')
    if (v) localStorage.removeItem('kettd.pending-route')
    return v
  } catch { return null }
}

/* ---------------------------------------------------------------- 展示辅助（M3 视图共用） */

/** formatDue：今天 HH:MM / 明天 / 周X / 逾期 */
export function formatDue(dueAt: string | null | undefined, today?: string): {
  text: string
  cls: string
  overdue: boolean
} {
  if (!dueAt) return { text: '无截止', cls: 'text-muted', overdue: false }
  const { date, time } = fullDueParts(dueAt)
  const t0 = today ?? localToday()
  const suffix = time ? ` ${time}` : ''
  if (date < t0) return { text: `逾期 · ${date.slice(5)}${suffix}`, cls: 'due-over', overdue: true }
  if (date === t0) return { text: `今天${suffix}`, cls: 'text-foreground', overdue: false }
  if (date === addDays(t0, 1)) return { text: `明天${suffix}`, cls: 'text-muted', overdue: false }
  const [y, m, d] = date.split('-').map(Number)
  const within7 = date <= addDays(t0, 7)
  const dow = WEEK_CN[new Date(y, m - 1, d).getDay()]
  return { text: within7 ? `${dow}${suffix}` : `${date.slice(5)}${suffix}`, cls: 'text-muted', overdue: false }
}

/** 展示色一律 token class，不硬编码颜色值 */
const CAT_CLS: Record<string, string> = { '工作': 'cat-work', '学习': 'cat-study', '生活': 'cat-life' }
export const getCategoryColor = (category: string) => CAT_CLS[category] ?? 'bg-muted text-muted'
const PRI_CLS: Record<string, string> = { high: 'pri-high', med: 'pri-med', low: 'pri-low' }
export const getPriorityColor = (priority: string) => PRI_CLS[priority] ?? 'pri-low'
export const PRIORITY_LABEL: Record<string, string> = { high: '高', med: '中', low: '低' }

/** 提醒时间校验：HH:MM（多时刻，循环）或完整时刻；返回 null = 合法，'__past__' = 已过去 */
export function validateReminderTime(s: string): string | null {
  const v = String(s ?? '').trim()
  if (!v) return '先选一个提醒时刻'
  const okHH = /^([01]\d|2[0-3]):[0-5]\d(\/([01]\d|2[0-3]):[0-5]\d)*$/.test(v)
  if (okHH) return null
  const okFull = /^\d{4}-\d{2}-\d{2}T([01]\d|2[0-3]):[0-5]\d$/.test(v)
  if (!okFull) return '时间还没选好，请重新选一次'
  return v < `${localToday()}T${localNowHHMM()}` ? '__past__' : null
}

/** 主题应用：settings.theme = light/dark（v1 旧值 "float" 不解读，视为 light） */
export function applyTheme(theme: string | undefined | null) {
  document.documentElement.classList.toggle('dark', theme === 'dark')
}
