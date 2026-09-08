import test from 'node:test';
import assert from 'node:assert/strict';
import { todaySections, inboxGroups, isPlannedToday } from '../src-react/src/kernel/selectors.js';

const TODAY = '2026-09-06';

function task(overrides = {}) {
  return {
    id: 't1',
    title: '示例',
    done: false,
    deletedAt: null,
    dueAt: null,
    plannedDate: null,
    carriedFrom: 0,
    createdAt: '2026-09-06T09:00',
    ...overrides,
  };
}

test('todaySections：今日到期 / 已拖到今天 / 逾期 三段互斥', () => {
  const tasks = [
    task({ id: 'due', dueAt: `${TODAY}T18:00` }),
    task({ id: 'planned', plannedDate: TODAY }),
    task({ id: 'carry', plannedDate: TODAY, carriedFrom: 2 }),
    task({ id: 'overdue', dueAt: '2026-09-04T09:00' }),
    task({ id: 'done', done: true, plannedDate: TODAY }),
    task({ id: 'trash', deletedAt: '2026-09-05T09:00' }),
  ];
  const s = todaySections(tasks, TODAY);
  assert.deepEqual(s.due.map((t) => t.id), ['due', 'planned'], '到点与计划归「今日」');
  assert.deepEqual(s.carried.map((t) => t.id), ['carry'], '带 carriedFrom 的粘留项归「已拖到今天」并可见拖了 N 天（PRD 6.3 AC）');
  assert.deepEqual(s.overdue.map((t) => t.id), ['overdue'], '逾期单独折叠');
  assert(!s.due.some((t) => t.id === 'done') && !s.overdue.some((t) => t.id === 'trash'));
});

test('todaySections：无截止无计划的 carried 任务归「已拖到今天」', () => {
  const s = todaySections([task({ id: 'c', carriedFrom: 3 })], TODAY);
  assert.deepEqual(s.carried.map((t) => t.id), ['c']);
  assert.equal(s.due.length, 0);
});

test('todaySections：逾期优先于一切分段', () => {
  const s = todaySections([task({ id: 'x', dueAt: '2026-09-01T09:00', plannedDate: TODAY, carriedFrom: 5 })], TODAY);
  assert.deepEqual(s.overdue.map((t) => t.id), ['x']);
  assert.equal(s.due.length, 0);
});

test('inboxGroups：今天进的 / 本周 / 更早（≥5 天）三组', () => {
  const tasks = [
    task({ id: 'fresh', createdAt: `${TODAY}T08:00` }),
    task({ id: 'd3', createdAt: '2026-09-03T08:00' }),
    task({ id: 'd5', createdAt: '2026-09-01T08:00' }),
    task({ id: 'd30', createdAt: '2026-08-07T08:00' }),
    task({ id: 'planned', plannedDate: TODAY }), // 不属收件箱
    task({ id: 'due', dueAt: '2026-09-08T09:00' }),
  ];
  const g = inboxGroups(tasks, TODAY);
  assert.deepEqual(g.fresh.map((e) => e.task.id), ['fresh']);
  assert.deepEqual(g.week.map((e) => e.task.id), ['d3'], '1–4 天归本周');
  assert.deepEqual(g.older.map((e) => e.task.id), ['d5', 'd30'], '≥5 天归更早');
  assert.equal(g.fresh[0].age, 0);
  assert.equal(g.older[0].age, 5);
});

test('inboxGroups：已完成/已计划/有截止的永不进组', () => {
  const g = inboxGroups(
    [task({ done: true }), task({ plannedDate: TODAY }), task({ dueAt: '2026-09-10T09:00' }), task({ deletedAt: 'x' })],
    TODAY,
  );
  assert.deepEqual(g.fresh, []);
  assert.deepEqual(g.week, []);
  assert.deepEqual(g.older, []);
});

test('todaySections 与 isPlannedToday 语义同源（同一份谓词）', () => {
  const t = task({ plannedDate: TODAY, carriedFrom: 1 });
  const s = todaySections([t], TODAY);
  assert.equal(s.carried.length, 1, '粘留项进顺延段');
  assert(isPlannedToday(t, TODAY), '同一份「今天」谓词，口径同源');
});
