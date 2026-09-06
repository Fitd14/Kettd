/**
 * 共享内核 · 任务选择器与"下一条提醒"推算（ADR-0006）
 *
 * 为什么在 kernel：谓词本身**并没有重复**（index.html 与 float.html 都调同一份 api 实现），
 * 搬过来的收益是另外两条 ——
 *   ① 给 src-react 一条干净的依赖边：视图层只依赖 kernel，不必拖着 Tauri 桥（ADR-0001 M3）；
 *   ② 补一个真的可测试性缺口：nextReminderTime 原先内部直调 localToday()/localNowHHMM()，
 *      无法注入时钟 → 跨午夜、weekly、workdays 这些边界一条都测不了。
 *      现在第三个参数可选注入 { today, now }，不传则用本地当前时间（调用点行为不变）。
 *
 * 三处「今天」语义必须同一 selector（V2-API §8 / brief 定性标准），所以这里只有一份。
 */
import { localToday, localNowHHMM, addDays, pad2 } from './time.js';

export function isOverdue(t, today) {
  const t0 = today || localToday();
  return !t.done && !!t.dueAt && String(t.dueAt).slice(0, 10) < t0;
}

/* 三处「今天」同一 selector（V2-API.md §8） */
export function isPlannedToday(t, today) {
  const t0 = today || localToday();
  return !t.done && !isOverdue(t, t0) && (t.plannedDate === t0 || String(t.dueAt || '').startsWith(t0));
}

export function isDoneToday(t, today) {
  const t0 = today || localToday();
  return !!t.done && (t.plannedDate === t0 || String(t.dueAt || '').startsWith(t0));
}

export function isCarry(t) { return (Number(t && t.carriedFrom) || 0) > 0; }

/* ISO 周（本地推算）：返回 '2026-W36' 及周一日期 */
export function isoWeekOf(isoDate) {
  const [y, m, d] = String(isoDate).slice(0, 10).split('-').map(Number);
  const dt = new Date(y, m - 1, d);
  const dow = (dt.getDay() + 6) % 7; // 周一=0
  const monday = new Date(y, m - 1, d - dow);
  const thu = new Date(monday.getFullYear(), monday.getMonth(), monday.getDate() + 3);
  const firstJan4 = new Date(thu.getFullYear(), 0, 4);
  const jan4Dow = (firstJan4.getDay() + 6) % 7;
  const week1Mon = new Date(firstJan4.getFullYear(), 0, 4 - jan4Dow);
  const week = Math.floor((monday.getTime() - week1Mon.getTime()) / (7 * 86400000)) + 1;
  const mm = String(monday.getMonth() + 1).padStart(2, '0');
  const dd = String(monday.getDate()).padStart(2, '0');
  return { week: `${thu.getFullYear()}-W${pad2(week)}`, monday: `${monday.getFullYear()}-${mm}-${dd}` };
}

export function weekWindow(which, today) {
  const t0 = today || localToday();
  const anchor = which === 'last' ? addDays(t0, -7) : t0;
  const { week, monday } = isoWeekOf(anchor);
  return { weekLabel: week, start: monday, end: addDays(monday, 6) };
}

function dowOf(dateStr) {
  const [y, m, d] = String(dateStr).slice(0, 10).split('-').map(Number);
  return new Date(y, m - 1, d).getDay();
}

function dayFires(repeat, refWeekday, dateStr) {
  const w = dowOf(dateStr);
  if (repeat === 'workdays') return w >= 1 && w <= 5;
  if (repeat === 'weekly') return refWeekday === null ? true : w === refWeekday;
  return true; // none(不循环) 与 daily 都按当天出现
}

/**
 * 下一条待响时刻（迷你条「下一条 HH:MM」与今天计数同源）。
 * @param {Array} tasks
 * @param {Array} reminders
 * @param {{today?: string, now?: string}} [clock] 注入时钟，仅测试用；缺省取本地当前
 */
export function nextReminderTime(tasks, reminders, clock) {
  const today = (clock && clock.today) || localToday();
  const now = (clock && clock.now) || localNowHHMM();
  const cands = [];
  const stamp = (day, time) => `${day}T${time}`;
  for (const t of tasks || []) {
    if (t.done || t.deletedAt) continue;
    const r = t.remindAt ? String(t.remindAt) : '';
    if (/^\d{2}:\d{2}$/.test(r)) { // HH:MM = 每日；仅对「今天要做/已逾期」的任务显示（迷你条语义）
      if (isPlannedToday(t, today) && r >= now) cands.push({ day: today, time: r });
    } else if (/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(r) && stamp(r.slice(0, 10), r.slice(11, 16)) >= stamp(today, now)) {
      cands.push({ day: r.slice(0, 10), time: r.slice(11, 16) });
    }
    const d = t.dueAt ? String(t.dueAt) : '';
    if (/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(d) && stamp(d.slice(0, 10), d.slice(11, 16)) >= stamp(today, now)) {
      cands.push({ day: d.slice(0, 10), time: d.slice(11, 16) });
    }
  }
  for (const r of reminders || []) {
    if (!r.enabled || r.completed) continue;
    const snz = r.snoozedUntil ? String(r.snoozedUntil) : '';
    if (/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(snz) && snz >= stamp(today, now)) { cands.push({ day: snz.slice(0, 10), time: snz.slice(11, 16) }); continue; }
    const ref = r.lastFired ? String(r.lastFired).slice(0, 10) : null;
    const refWd = ref ? dowOf(ref) : null;
    for (let p of String(r.time || '').split('/')) {
      p = p.trim();
      if (/^\d{2}:\d{2}$/.test(p)) {
        if (r.repeat === 'none') { if (p >= now) cands.push({ day: today, time: p }); continue; }
        for (let off = 0; off <= 7; off++) {
          const day = addDays(today, off);
          if (off === 0 && p < now) continue;
          if (dayFires(r.repeat, refWd, day)) { cands.push({ day, time: p }); break; }
        }
      } else if (/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(p) && p >= stamp(today, now)) {
        cands.push({ day: p.slice(0, 10), time: p.slice(11, 16) });
      }
    }
  }
  if (!cands.length) return null;
  cands.sort((a, b) => stamp(a.day, a.time).localeCompare(stamp(b.day, b.time)));
  const n = cands[0];
  return n.day === today ? n.time : `${n.day.slice(5)} ${n.time}`;
}

/**
 * 「今天」视图三段式（PRD 6.3 / today-list-ui-spec）：今日到期 / 已拖到今天 / 逾期折叠。
 * 逾期项不混入主体，收进折叠行由视图渲染计数；全部输入排除回收站与已完成。
 * @param {Array} tasks
 * @param {string} today 注入今天（YYYY-MM-DD），测试必传；缺省取本地
 * @returns {{ due: Array, carried: Array, overdue: Array }}
 */
export function todaySections(tasks, today) {
  const t0 = today || localToday();
  const live = (t) => !t.done && !t.deletedAt;
  const due = [];
  const carried = [];
  const overdue = [];
  for (const t of tasks || []) {
    if (!live(t)) continue;
    if (isOverdue(t, t0)) { overdue.push(t); continue; }
    // 顺延优先于今日段：自动粘留的条目 plannedDate 已是今天，但「拖了 N 天」必须可见（PRD 6.3 AC）
    const onToday = t.plannedDate === t0 || String(t.dueAt || '').startsWith(t0);
    if ((Number(t.carriedFrom) || 0) > 0) { carried.push(t); continue; }
    if (onToday) { due.push(t); continue; }
  }
  return { due, carried, overdue };
}

/**
 * 收件箱老化分组（inbox-ui-spec ③）：今天进的 / 本周 / 更早（≥5 天滞留）。
 * 收件箱定义沿用现状锚点：未完成、无 plannedDate、无 dueAt、不在回收站。
 *滞留天数 = today − createdAt（按日差，当天为 0）。
 * @returns {{ fresh: Array, week: Array, older: Array }} 每项为 { task, age }
 */
export function inboxGroups(tasks, today) {
  const t0 = today || localToday();
  const live = (t) => !t.done && !t.deletedAt && !t.plannedDate && !t.dueAt;
  const groups = { fresh: [], week: [], older: [] };
  for (const t of tasks || []) {
    if (!live(t)) continue;
    const created = String(t.createdAt || '').slice(0, 10);
    const age = created && created < t0
      ? Math.round((new Date(t0) - new Date(created)) / 86400000)
      : 0;
    const entry = { task: t, age };
    if (age <= 0) groups.fresh.push(entry);
    else if (age < 5) groups.week.push(entry);
    else groups.older.push(entry);
  }
  return groups;
}
