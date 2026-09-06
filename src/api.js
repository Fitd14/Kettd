/* Kettd v2 · Tauri v1 前端桥接层。
 * 契约真相源：src-tauri/docs/V2-API.md —— 40 个命令逐一对应，参数名一律 camelCase。
 * 约定：所有命令封装返回 { data, err }；err 为后端可直接展示的中文短句，永不 throw。
 * 时间全链路本地语义字符串，零 UTC 换算（不用 toISOString）。
 *
 * 纯逻辑（本地日期原语、快录解析与组参）已收进 src/kernel/（ADR-0006）：
 * 本文件只保留 Tauri 桥接与命令封装，并把内核符号再导出以保持所有 api.xxx 调用点不变。 */
import { pad2, WEEK_CN, localToday, localNowHHMM, addDays, isLocalDate, fullDueParts } from './kernel/time.js';
import { parseCapture } from './kernel/capture.js';
import { createEventBus } from './kernel/event-bus.js';
import { isOverdue, isPlannedToday, isDoneToday, isCarry, isoWeekOf, weekWindow, nextReminderTime } from './kernel/selectors.js';

export { pad2, WEEK_CN, localToday, localNowHHMM, addDays, isLocalDate, fullDueParts } from './kernel/time.js';
export { parseCapture, buildCaptureArgs } from './kernel/capture.js';
export { isOverdue, isPlannedToday, isDoneToday, isCarry, isoWeekOf, weekWindow, nextReminderTime } from './kernel/selectors.js';

const T = globalThis.__TAURI__ || {};
const rawInvoke = (T.tauri && T.tauri.invoke) || (T.core && T.core.invoke) || T.invoke;
const rawListen = (T.event && T.event.listen) || T.listen;
const rawEmit = (T.event && T.event.emit) || T.emit;

async function call(command, args) {
  if (typeof rawInvoke !== 'function') return { data: null, err: '未连接桌面运行时，命令未执行' };
  try {
    const data = await rawInvoke(command, args || {});
    return { data: data === undefined ? null : data, err: null };
  } catch (e) {
    const err = typeof e === 'string' ? e : (e && (e.message || e.payload)) || String(e);
    return { data: null, err };
  }
}

/* ---------------- 命令封装（37，与 V2-API.md §2 表逐一对应） ---------------- */

export async function getBootstrap() { return call('get_bootstrap'); }
export async function getTasks(includeDeleted) { return call('get_tasks', { includeDeleted: !!includeDeleted }); }
export async function getTask(id) { return call('get_task', { id }); }
export async function addTask(args) { return call('add_task', { args }); }
export async function updateTask(id, patch) { return call('update_task', { id, patch }); }
export async function toggleTask(id) { return call('toggle_task', { id }); }
export async function deleteTask(id) { return call('delete_task', { id }); }
export async function undoDelete() { return call('undo_delete'); }
export async function restoreTask(id) { return call('restore_task', { id }); }
export async function purgeTask(id) { return call('purge_task', { id }); }
export async function addSubtask(taskId, title) { return call('add_subtask', { taskId, title }); }
export async function toggleSubtask(taskId, subtaskId) { return call('toggle_subtask', { taskId, subtaskId }); }
export async function deleteSubtask(taskId, subtaskId) { return call('delete_subtask', { taskId, subtaskId }); }
export async function addNote(taskId, note) { return call('add_note', { taskId, note }); }
export async function deleteNote(taskId, noteId) { return call('delete_note', { taskId, noteId }); }
export async function getReminders() { return call('get_reminders'); }
export async function addReminder(reminder) { return call('add_reminder', { reminder }); }
export async function updateReminder(id, patch) { return call('update_reminder', { id, patch }); }
export async function deleteReminder(id) { return call('delete_reminder', { id }); }
export async function toggleReminder(id) { return call('toggle_reminder', { id }); }
export async function snoozeReminder(id, minutes) { return call('snooze_reminder', { id, minutes }); }
export async function getSettings() { return call('get_settings'); }
export async function setSettings(patch) { return call('set_settings', { patch }); }
export async function openMainWindow() { return call('open_main_window'); }
export async function showFloat() { return call('show_float'); }
export async function hideFloat() { return call('hide_float'); }
export async function openDataFolder() { return call('open_data_folder'); }
export async function setStickyPinned(pinned) { return call('set_sticky_pinned', { pinned }); }
export async function registerCaptureHotkey(combo) { return call('register_capture_hotkey', { combo }); }
export async function registerMainHotkey(combo) { return call('register_main_hotkey', { combo }); }
/** 热键实际注册快照 { capture, main }（null = 未绑上/已解绑），设置页据此标「未生效 */
export async function getHotkeyStatus() { return call('get_hotkey_status'); }
export async function openCaptureOverlay() { return call('open_capture_overlay'); }
export async function closeCaptureOverlay() { return call('close_capture_overlay'); }
export async function exportWeekly(opts) {
  const o = opts || {};
  const args = {};
  if (o.week != null) args.week = o.week;
  if (o.format != null) args.format = o.format;
  if (o.dir != null) args.dir = o.dir;
  return call('export_weekly', args);
}
export async function getBackups() { return call('get_backups'); }
export async function restoreBackup(slot) { return call('restore_backup', { slot }); }
export async function getDataHealth() { return call('get_data_health'); }
export async function clearMigrationReport() { return call('clear_migration_report'); }
export async function getFormHints() { return call('get_form_hints'); }

/* ---------------- 事件（补发队列语义已收进内核 event-bus.js，ADR-0006/E14 唯一实现） ---------------- */

const bus = createEventBus({ listen: rawListen });
export const onEvent = bus.onEvent;
export const eventOnce = bus.eventOnce;

/* 跨窗导航：优先走后端事件总线；不可用时落同一 webview 源的 localStorage，主窗 focus 时自取 */
export function emitNavigate(route) {
  if (typeof rawEmit === 'function') {
    try { const p = rawEmit('navigate', route); if (p && p.catch) p.catch(() => persistRoute(route)); return; }
    catch (_) { /* fall through */ }
  }
  persistRoute(route);
}
function persistRoute(route) {
  try { localStorage.setItem('kettd.pending-route', String(route)); } catch (_) { /* 隔离环境忽略 */ }
}
export function takePendingRoute() {
  try {
    const v = localStorage.getItem('kettd.pending-route');
    if (v) localStorage.removeItem('kettd.pending-route');
    return v;
  } catch (_) { return null; }
}

/* ---------------- 本地语义时间（与后端约定对齐：YYYY-MM-DD / YYYY-MM-DDTHH:mm / HH:MM） ---------------- */

/* 本地日期原语（pad2 / WEEK_CN / localToday / localNowHHMM / addDays / isLocalDate /
   fullDueParts）已搬到 src/kernel/time.js，文件头统一导入并再导出。 */

/* formatDue：今天 HH:MM / 明天 / 周X / 逾期红（cls=due-over 由调用方挂到 .due-over） */
export function formatDue(dueAt, today) {
  if (!dueAt) return { text: '无截止', cls: 'text-muted', overdue: false };
  const { date, time } = fullDueParts(dueAt);
  const t0 = today || localToday();
  const suffix = time ? ` ${time}` : '';
  if (date < t0) return { text: `逾期 · ${date.slice(5)}${suffix}`, cls: 'due-over', overdue: true };
  if (date === t0) return { text: `今天${suffix}`, cls: 'text-foreground', overdue: false };
  if (date === addDays(t0, 1)) return { text: `明天${suffix}`, cls: 'text-muted', overdue: false };
  const [y, m, d] = date.split('-').map(Number);
  const within7 = date <= addDays(t0, 7);
  const dow = WEEK_CN[new Date(y, m - 1, d).getDay()];
  return {
    text: within7 ? `${dow}${suffix}` : `${date.slice(5)}${suffix}`,
    cls: 'text-muted', overdue: false,
  };
}
/* 任务选择器与周窗口已搬到 src/kernel/selectors.js（ADR-0006）：谓词本就只有一份，
   搬走是为了给 src-react 干净的依赖边，并让 nextReminderTime 可注入时钟做测试。
   文件头已再导出，api.xxx 调用点不变。 */

/* ---------------- 展示色：一律 token class，不硬编码任何颜色值 ---------------- */

const CAT_CLS = { '工作': 'cat-work', '学习': 'cat-study', '生活': 'cat-life' };
export function getCategoryColor(category) { return CAT_CLS[category] || 'bg-muted text-muted'; }
const PRI_CLS = { high: 'pri-high', med: 'pri-med', low: 'pri-low' };
export function getPriorityColor(priority) { return PRI_CLS[priority] || 'pri-low'; }
export const PRIORITY_LABEL = { high: '高', med: '中', low: '低' };

/* esc 的实现已收进共享内核（ADR-0006）：vanilla 三窗与 src-react 共用同一份。
   这里再导出以保持所有 api.esc(...) 调用点不变。 */
export { esc } from './kernel/text.js';

/* ---------------- 快录语法解析与组参 ----------------
   parseCapture / buildCaptureArgs 已搬到 src/kernel/capture.js（ADR-0006）：
   原先快录组参在 index.html / float.html / capture.html 各有一份逐字复制，
   现由内核的 buildCaptureArgs 唯一提供。文件头已再导出，调用点不变。 */
/* 提醒时间校验：HH:MM（多时刻 / 分隔，循环语义，永不“过期”）或 YYYY-MM-DDTHH:mm（对齐 V2-API 时间语义） */
export function validateReminderTime(s) {
  const v = String(s || '').trim();
  if (!v) return '先选一个提醒时刻';
  const okHH = /^([01]\d|2[0-3]):[0-5]\d(\/([01]\d|2[0-3]):[0-5]\d)*$/.test(v);
  if (okHH) return null;
  const okFull = /^\d{4}-\d{2}-\d{2}T([01]\d|2[0-3]):[0-5]\d$/.test(v);
  if (!okFull) return '时间还没选好，请重新选一次';
  return v < `${localToday()}T${localNowHHMM()}` ? '__past__' : null;
}

/* 下一条待响时刻（float/迷你条「下一条 HH:MM」，与今天计数同源，全本地比较） */
export function debounce(fn, ms) {
  let timer = null;
  return (...a) => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => { timer = null; fn(...a); }, ms);
  };
}

/* 主题应用：settings.theme = light/dark（v1 旧值 "float" 不解读，视为 light） */
export function applyTheme(theme) {
  document.documentElement.classList.toggle('dark', theme === 'dark');
}
