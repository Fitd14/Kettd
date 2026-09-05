/**
 * 共享内核 · 本地日期原语（ADR-0006）
 *
 * 规则：纯函数、零全局、零 Tauri。vanilla 三窗与 src-react 共用同一份实现，
 * node 测试可直接 import（api.js 只做再导出，不重复实现）。
 * 时间全链路本地语义字符串，零 UTC 换算（不用 toISOString）—— 对治 v1 audit-6 时间漂移。
 */

export const pad2 = (n) => String(n).padStart(2, '0');

export const WEEK_CN = ['周日', '周一', '周二', '周三', '周四', '周五', '周六'];

/** 今天的本地语义日期 YYYY-MM-DD */
export function localToday() {
  const d = new Date();
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
}

/** 当前本地时刻 HH:MM */
export function localNowHHMM() {
  const d = new Date();
  return `${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
}

/** 日期加减 n 天（按日历日，不做时区运算） */
export function addDays(iso, n) {
  const [y, m, d] = String(iso).slice(0, 10).split('-').map(Number);
  const dt = new Date(y, m - 1, d + n);
  return `${dt.getFullYear()}-${pad2(dt.getMonth() + 1)}-${pad2(dt.getDate())}`;
}

export function isLocalDate(s) {
  return /^\d{4}-\d{2}-\d{2}$/.test(String(s || ''));
}

/** 拆开 YYYY-MM-DDTHH:mm；仅有日期时 time 为空串 */
export function fullDueParts(dueAt) {
  const date = String(dueAt).slice(0, 10);
  const time = dueAt && String(dueAt).length > 10 ? String(dueAt).slice(11, 16) : '';
  return { date, time };
}
