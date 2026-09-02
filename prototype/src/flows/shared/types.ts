// 共享类型：Kettd v2 原型（对应 stories.json / sitemap.json）

export type Priority = 'high' | 'med' | 'low'
export type Category = '工作' | '学习' | '生活'

export interface Subtask {
  id: string
  title: string
  done: boolean
}

export interface Task {
  id: string
  title: string
  note?: string
  category: Category
  priority: Priority
  /** 截止 ISO datetime（本地语义），无则视为收件箱项 */
  dueAt?: string
  /** 提醒时刻 ISO，null = 不提醒 */
  remindAt?: string | null
  /** 今日计划：被主动拉进"今天"的日期，null = 未计划 */
  plannedDate?: string | null
  /** 从哪天粘留过来（>0 = 拖了 N 天） */
  carriedFromDays?: number
  done: boolean
  /** 完成时刻（进日志簿的锚点） */
  doneAt?: string | null
  subtasks: Subtask[]
  createdAt: string
  source: 'capture' | 'manual' | 'seed'
}

export type ChipKind = 'date' | 'time' | 'category' | 'priority'

export interface ParseChip {
  kind: ChipKind
  /** 原始命中文本 */
  raw: string
  /** 解析后的可读值 */
  value: string
  /** 置信度：high 直接应用；low 需用户确认 */
  confidence: 'high' | 'low'
}

export type FloatForm = 'alwaysOnTop' | 'embedded' | 'mini'

export interface ReminderEvent {
  id: string
  taskId: string
  fireAt: string
  title: string
  state: 'pending' | 'fired' | 'snoozed'
  snoozeTo?: string
}

export interface LogEntry {
  taskId: string
  title: string
  category: Category
  doneAt: string
}

export interface WeeklyDraft {
  week: string
  generated: boolean
  done: LogEntry[]
  carriedOver: Task[]
  overdueHandled: Task[]
  categoryBreakdown: Record<Category, number>
  edited: boolean
  exportedPath?: string
}

export interface BackupSnapshot {
  id: string
  createdAt: string
  sizeKB: number
  taskCount: number
}

export type DataHealth = 'ok' | 'corrupt' | 'writeFailed'
