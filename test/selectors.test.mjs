/**
 * 任务选择器 / 下一条提醒推算 · 回归测试（ADR-0006）
 *
 * 这批用例是搬移的**理由本身**：nextReminderTime 原先内部直调 localToday()/localNowHHMM()，
 * 无法注入时钟，跨午夜 / weekly / workdays 这些边界一条都测不了。
 * 现在第三个参数注入 { today, now }，所有断言都与"跑测试的真实时刻"无关。
 *
 * 跑法：node test/selectors.test.mjs
 */
import assert from 'node:assert/strict';
import {
  isOverdue, isPlannedToday, isDoneToday, isCarry, isoWeekOf, weekWindow, nextReminderTime,
} from '../src-react/src/kernel/selectors.js';

let pass = 0;
const it = (name, fn) => { fn(); pass++; console.log('  ✓', name); };
// 2026-09-05 周六 → 09-06 周日、09-07 周一、09-09 周三
const SUN = { today: '2026-09-06', now: '10:00' };
const task = (o) => ({ id: 'x', title: 't', done: false, ...o });
const rem = (o) => ({ id: 'r', title: 'R', enabled: true, completed: false, repeat: 'none', ...o });
const next = (tasks, reminders, clock) => nextReminderTime(tasks, reminders, clock || SUN);

console.log('「今天」三态互斥（brief 定性标准：三处同一口径）');
it('逾期项不再算今天', () => {
  const t = task({ dueAt: '2026-09-01T09:00' });
  assert.equal(isOverdue(t, '2026-09-06'), true);
  assert.equal(isPlannedToday(t, '2026-09-06'), false, '逾期即不再算今天，避免同一项被计两次');
});
it('计划日命中 / 截止日在今天 都算今天', () => {
  assert.equal(isPlannedToday(task({ plannedDate: '2026-09-06' }), '2026-09-06'), true);
  assert.equal(isPlannedToday(task({ dueAt: '2026-09-06T18:00' }), '2026-09-06'), true);
});
it('已完成项不算今天待办，但算今天完成', () => {
  const t = task({ done: true, plannedDate: '2026-09-06' });
  assert.equal(isPlannedToday(t, '2026-09-06'), false);
  assert.equal(isDoneToday(t, '2026-09-06'), true);
});
it('isCarry 只认正数拖留天数', () => {
  assert.equal(isCarry(task({ carriedFrom: 3 })), true);
  assert.equal(isCarry(task({ carriedFrom: 0 })), false);
  assert.equal(isCarry(task({ carriedFrom: '2' })), true);
  assert.equal(isCarry(null), false);
});

console.log('nextReminderTime · 注入时钟的边界（原先测不了的部分）');
it('每日提醒当天时刻已过 → 顺延到明天', () => {
  assert.equal(next([], [rem({ time: '09:00', repeat: 'daily' })]), '09-07 09:00');
});
it('repeat=none 的循环时刻已过 → 不产出（不拿它冒充未来）', () => {
  assert.equal(next([], [rem({ time: '09:00', repeat: 'none' })]), null);
  assert.equal(next([], [rem({ time: '11:00', repeat: 'none' })]), '11:00');
});
it('工作日提醒在周日 → 落到周一', () => {
  assert.equal(next([], [rem({ time: '08:30', repeat: 'workdays' })]), '09-07 08:30');
});
it('每周提醒按 lastFired 的星期对齐（上次周三 → 下个周三）', () => {
  const r = rem({ time: '15:00', repeat: 'weekly', lastFired: '2026-09-02T15:00' }); // 9-02 是周三
  assert.equal(next([], [r]), '09-09 15:00');
});
it('多时刻取最近的那个', () => {
  assert.equal(next([], [rem({ time: '09:00/11:30/16:00', repeat: 'daily' })]), '11:30');
});
it('稍后提醒（snooze）优先于常规排期', () => {
  const r = rem({ time: '09:00', repeat: 'daily', snoozedUntil: '2026-09-06T10:30' });
  assert.equal(next([], [r]), '10:30');
});
it('跨午夜不依赖真实时钟：23:59 时每日 00:05 落到明天', () => {
  const late = { today: '2026-09-06', now: '23:59' };
  assert.equal(next([], [rem({ time: '00:05', repeat: 'daily' })], late), '09-07 00:05');
});
it('停用或已完成的提醒不参与', () => {
  assert.equal(next([], [rem({ enabled: false, time: '11:00', repeat: 'daily' })]), null);
  assert.equal(next([], [rem({ completed: true, time: '11:00', repeat: 'daily' })]), null);
});
it('任务每日提醒仅对「今天要做」的项显示（迷你条语义）', () => {
  const dueToday = [task({ plannedDate: '2026-09-06', remindAt: '11:00' })];
  const notToday = [task({ plannedDate: '2026-09-10', remindAt: '11:00' })];
  assert.equal(next(dueToday, []), '11:00');
  assert.equal(next(notToday, []), null);
});
it('已完成与回收站任务被排除', () => {
  assert.equal(next([task({ done: true, dueAt: '2026-09-06T11:00' })], []), null);
  assert.equal(next([task({ deletedAt: '2026-09-06T09:00', dueAt: '2026-09-06T11:00' })], []), null);
});
it('带日期的任务提醒与截止共同参与排序取最近', () => {
  const tasks = [
    task({ remindAt: '2026-09-08T08:00' }),
    task({ dueAt: '2026-09-07T20:00' }),
  ];
  assert.equal(next(tasks, []), '09-07 20:00');
});
it('空输入与脏字段不炸', () => {
  assert.equal(next([], []), null);
  assert.equal(next(undefined, undefined), null);
  assert.equal(next([task({ time: '坏值' })], [rem({ time: '' })]), null);
});

console.log('ISO 周（回顾页周期口径）');
it('跨年周归到正确年份（ISO 规则：以该周周四所在年为准）', () => {
  // 2026-01-01 是周四 → 2026 为 53 周的长周年
  assert.equal(isoWeekOf('2026-01-01').week, '2026-W01');
  assert.equal(isoWeekOf('2026-12-28').week, '2026-W53', '长周年的最后一周仍归 2026');
  // 2027-01-01 是周五，它所在周的周四是 2026-12-31 → 仍属 2026-W53
  assert.equal(isoWeekOf('2027-01-01').week, '2026-W53', '年初几天可归属上一年最后一周');
  assert.equal(isoWeekOf('2027-01-04').week, '2027-W01', '2027-W01 从 01-04 周一才开始');
  assert.equal(isoWeekOf('2026-01-04').week, '2026-W01');
});
it('weekWindow 给周一到周日七天', () => {
  const w = weekWindow('current', '2026-09-06');
  assert.equal(w.start, '2026-08-31');
  assert.equal(w.end, '2026-09-06');
  assert.equal(weekWindow('last', '2026-09-06').end, '2026-08-30');
});

console.log(`\n${pass} 项全绿`);
