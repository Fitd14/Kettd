import type { Task, WeeklyDraft, BackupSnapshot, ReminderEvent } from './types'

// 基准"今天"：2026-09-02（周三）。全部本地语义时间，不混 UTC——story-1 的时区修复在 mock 层先示范
export const TODAY = '2026-09-02'

const t = (
  id: string, title: string, extra: Partial<Task> = {},
): Task => ({
  id, title, category: '生活', priority: 'med', done: false,
  plannedDate: null, carriedFromDays: 0, subtasks: [], source: 'seed',
  remindAt: null, dueAt: undefined, doneAt: null, note: undefined, createdAt: '2026-08-28',
  ...extra,
})

export const seedTasks: Task[] = [
  // —— 已计划进今天
  t('t-01', '整理季度复盘初稿', { category: '工作', priority: 'high', dueAt: '2026-09-02 17:00', remindAt: '2026-09-02 16:30', plannedDate: TODAY }),
  t('t-02', '高数第六章 3 道课后题', { category: '学习', priority: 'med', dueAt: '2026-09-02 21:00', remindAt: '2026-09-02 20:00' , plannedDate: TODAY }),
  t('t-03', '给家里回电话', { priority: 'low', plannedDate: TODAY, carriedFromDays: 2 }),
  t('t-04', '取快递（丰巢）', { priority: 'low', dueAt: '2026-09-02 19:00', plannedDate: TODAY, carriedFromDays: 1 }),
  // —— 今日已完成（进日志）
  t('t-05', '晨间背 30 个单词', { done: true, doneAt: '2026-09-02 07:40', plannedDate: TODAY }),
  // —— 收件箱（未整理捕获）
  t('t-06', '买路由器', { source: 'capture', createdAt: '2026-09-01' }),
  t('t-07', '约导师改开题', { source: 'capture', createdAt: '2026-09-01' }),
  t('t-08', '续 gym 卡', { source: 'capture', createdAt: '2026-08-31' }),
  // —— 逾期（折叠行）
  t('t-09', '提交发票报销单', { category: '工作', priority: 'high', dueAt: '2026-08-29 12:00', carriedFromDays: 4 }),
  t('t-10', '视力复查预约', { dueAt: '2026-08-30 18:00', carriedFromDays: 3 }),
  // —— 未来计划
  t('t-11', '周日下午 线上组会准备', { category: '学习', plannedDate: null, dueAt: '2026-09-06 14:00', remindAt: '2026-09-06 13:30' }),
]

export const seedReminders: ReminderEvent[] = [
  { id: 'r-01', taskId: 't-01', fireAt: '2026-09-02 16:30', title: '该写季度复盘了（还剩 30 分钟到 17:00）', state: 'pending' },
  { id: 'r-02', taskId: 't-02', fireAt: '2026-09-02 20:00', title: '高数练习题 · 21:00 前完成', state: 'pending' },
  { id: 'r-03', taskId: 't-11', fireAt: '2026-09-06 13:30', title: '组会准备提醒', state: 'pending' },
]

// 上周日志（story-6 周五复盘用）：模拟 8/24(一)-8/28(五) 一周
export const lastWeekLogs = [
  { taskId: 'x-01', title: '完成开题 PPT 初版', category: '学习' as const, doneAt: '2026-08-24 21:30' },
  { taskId: 'x-02', title: '报销单模板找财务确认', category: '生活' as const, doneAt: '2026-08-24 15:12' },
  { taskId: 'x-03', title: '周报按时提交', category: '工作' as const, doneAt: '2026-08-25 17:02' },
  { taskId: 'x-04', title: '取图书馆还书', category: '生活' as const, doneAt: '2026-08-26 13:40' },
  { taskId: 'x-05', title: '高数月考错题订正', category: '学习' as const, doneAt: '2026-08-26 22:15' },
  { taskId: 'x-06', title: '和主管对齐 Q3 里程碑', category: '工作' as const, doneAt: '2026-08-27 10:30' },
  { taskId: 'x-07', title: '续 gym 卡询价（未完成→收件箱）', category: '生活' as const, doneAt: '2026-08-28 19:00' },
  { taskId: 'x-08', title: '整理 8 月支出', category: '生活' as const, doneAt: '2026-08-28 20:45' },
]

export const seedWeeklyDraft: WeeklyDraft = {
  week: '2026-W35（08/24 – 08/30）',
  generated: false,
  done: lastWeekLogs,
  carriedOver: [
    t('c-01', '报销单提交（上周顺延）', { category: '工作', carriedFromDays: 3, dueAt: '2026-08-29 12:00' }),
  ],
  overdueHandled: [
    t('o-01', '清理旧文档归档', { dueAt: '2026-08-27 18:00' }),
  ],
  categoryBreakdown: { 工作: 1, 学习: 3, 生活: 4 },
  edited: false,
}

export const seedBackups: BackupSnapshot[] = [
  { id: 'b-3', createdAt: '2026-09-02 09:12', sizeKB: 41, taskCount: 11 },
  { id: 'b-2', createdAt: '2026-09-01 22:40', sizeKB: 39, taskCount: 9 },
  { id: 'b-1', createdAt: '2026-08-31 08:05', sizeKB: 38, taskCount: 8 },
]

/**
 * 快录语法解析（受限四类 token：日期词 / 时间 / #分类 / !优先级）
 * 契约：输出全部为结构化本地值 dueAt 'YYYY-MM-DD' 或 'YYYY-MM-DDTHH:mm'
 *（本地语义，零 UTC 换算，对齐 PRD 6.6 / M1 修复）；识别失败不拦截，纯文本照收。
 */
export interface CaptureParseResult {
  title: string
  chips: import('./types').ParseChip[]
  dueAt: string | null
  autoRemind: boolean
  category?: Task['category']
  priority?: Task['priority']
  fellBack: boolean
}

const pad = (n: number) => String(n).padStart(2, '0')

export function parseCapture(raw: string, today: string = TODAY): CaptureParseResult {
  let text = raw.trim()
  const chips: import('./types').ParseChip[] = []
  let category: Task['category'] | undefined
  let priority: Task['priority'] | undefined

  const cat = text.match(/#(工作|学习|生活)/)
  if (cat) {
    category = cat[1] as Task['category']
    chips.push({ kind: 'category', raw: cat[0], value: `分类 · ${category}`, confidence: 'high' })
    text = text.replace(cat[0], ' ')
  }

  const prio = text.match(/!(高|中|低)/)
  if (prio) {
    const pmap: Record<string, Task['priority']> = { 高: 'high', 中: 'med', 低: 'low' }
    priority = pmap[prio[1]]
    chips.push({ kind: 'priority', raw: prio[0], value: `优先级 · ${prio[1]}`, confidence: 'high' })
    text = text.replace(prio[0], ' ')
  }

  // —— 时间：下午3点 / 15:00 / 晚上9点半 / 中午12点
  let hour: number | null = null
  let minute = 0
  const tm = text.match(/(早上|上午|中午|下午|晚上)\s*(\d{1,2})\s*[:：点]\s*(半|整|\d{1,2})?|(\d{1,2})[:：](\d{2})/)
  if (tm) {
    if (tm[4] !== undefined) {
      hour = parseInt(tm[4], 10)
      minute = parseInt(tm[5], 10)
    } else {
      hour = parseInt(tm[2], 10)
      const m = tm[3]
      minute = m === '半' ? 30 : m && m !== '整' ? parseInt(m, 10) : parseInt(m ?? '', 10) || 0
      const ap = tm[1]
      if ((ap === '下午' || ap === '晚上') && hour < 12) hour += 12
      if (ap === '中午' && hour < 11) hour += 12
    }
    if (hour > 23 || minute > 59) {
      hour = null
      minute = 0
    } else {
      chips.push({ kind: 'time', raw: tm[0].trim(), value: `时间 · ${pad(hour)}:${pad(minute)}`, confidence: 'high' })
      text = text.replace(tm[0], ' ')
    }
  }

  // —— 日期：今天/明天/后天 / 周X / 下周X / N月D日 / N.D
  const [ty, tmo, tda] = today.split('-').map(Number)
  const base = new Date(ty, tmo - 1, tda)
  let dateISO: string | null = null
  const dm = text.match(/(今天|明天|后天)/)
    || text.match(/(本周|下周|星期|周)([一二三四五六日天1-7])/)
    || text.match(/(\d{1,2})月(\d{1,2})[日号]/)
    || text.match(/(\d{1,2})[./](\d{1,2})(?=\s|$|[^(\d])/)
  if (dm) {
    const d = new Date(base)
    if (/^(今天|明天|后天)$/.test(dm[0])) {
      const off: Record<string, number> = { 今天: 0, 明天: 1, 后天: 2 }
      d.setDate(base.getDate() + off[dm[0]])
    } else if (dm[2] !== undefined && /^(本周|下周|星期|周)/.test(dm[0])) {
      const wmap: Record<string, number> = { 一: 1, 二: 2, 三: 3, 四: 4, 五: 5, 六: 6, 日: 0, 天: 0, '1': 1, '2': 2, '3': 3, '4': 4, '5': 5, '6': 6, '7': 0 }
      const target = wmap[dm[2]] ?? base.getDay()
      let diff = (target - base.getDay() + 7) % 7
      if (dm[0].startsWith('下周')) diff += 7
      d.setDate(base.getDate() + diff)
    } else {
      const nums = (dm[0].match(/\d+/g) ?? []).map(Number)
      if (nums.length === 2) d.setMonth(nums[0] - 1, nums[1])
    }
    dateISO = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
    chips.push({ kind: 'date', raw: dm[0], value: `日期 · ${dateISO}`, confidence: 'high' })
    text = text.replace(dm[0], ' ')
  }

  let dueAt: string | null = dateISO
  if (dueAt && hour !== null) dueAt = `${dueAt}T${pad(hour)}:${pad(minute)}`
  else if (!dueAt && hour !== null) {
    // 只有时间：视为今天该时刻
    dueAt = `${today}T${pad(hour)}:${pad(minute)}`
  }

  const titleFallback = text.replace(/\s+/g, ' ').trim()
  return {
    title: titleFallback || raw.trim(),
    chips,
    dueAt,
    autoRemind: dueAt !== null, // 有截止即默认随截止提醒（可在编辑中关闭，PRD 6.4）
    category,
    priority,
    fellBack: chips.length === 0,
  }
}
