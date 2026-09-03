/**
 * 提醒时刻编辑器回归测试（v2.1 · T3）
 *
 * 不重写逻辑：直接从 src/index.html 里把真实函数源码抽出来求值，
 * 所以改坏了 index.html，这个测试就会红。
 * 覆盖：qa-3（v1 遗留多时刻不得静默降级为单时刻）、qa-2（文案不再教手输）、
 *       Ask 1（显式「每天/单次」开关的收敛行为）。
 *
 * 跑法：node test/rem-editor.test.mjs   （零依赖，非零退出码即失败）
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import assert from 'node:assert/strict';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const html = readFileSync(path.join(root, 'src', 'index.html'), 'utf8');

/* 大括号配平截取一段完整定义（模板串里的 ${} 成对出现，配平成立） */
function sliceFrom(idx) {
  const open = html.indexOf('{', idx);
  assert.ok(open > idx, '找不到函数体起点');
  let depth = 0;
  for (let j = open; j < html.length; j++) {
    if (html[j] === '{') depth++;
    else if (html[j] === '}') { depth--; if (depth === 0) return html.slice(idx, j + 1); }
  }
  throw new Error('大括号不配平');
}
const grabFn = (name) => {
  const i = html.indexOf(`function ${name}(`);
  assert.ok(i >= 0, `src/index.html 里找不到 function ${name}`);
  return sliceFrom(i);
};
const grabAct = (key) => {
  const i = html.indexOf(`ACTS['${key}'] =`);
  assert.ok(i >= 0, `src/index.html 里找不到 ACTS['${key}']`);
  return sliceFrom(i);
};

const S = { drafts: {}, errs: {}, reminders: [], remEdit: null };
const ctx = {
  S,
  esc: (x) => String(x),
  renderView: () => { ctx.rendered++; },
  rendered: 0,
  document: { querySelector: () => null },
};
const load = (src) => new Function(
  'S', 'esc', 'renderView', 'document',
  `${src}\n; return { remCompose, remTimeEditor, setRemMode, act: ACTS };`,
)(S, ctx.esc, ctx.renderView, ctx.document);
const body = [grabFn('remCompose'), grabFn('remTimeEditor'), grabFn('setRemMode'),
  `const ACTS = {};`, grabAct('rem-edit')].join('\n');
const { remCompose, remTimeEditor, setRemMode, act } = load(body);

const daily = (...times) => ({ remMode: 'daily', remTimes: times, remDate: '' });
let pass = 0;
const ok = (name, fn) => { fn(); pass++; console.log('  ✓', name); };

console.log('remCompose · 每天=多时刻串 / 单次=带日期');
ok('每天 · 两个时刻 → HH:MM/HH:MM', () => {
  S.drafts = daily('09:00', '14:00');
  assert.equal(remCompose('rem').value, '09:00/14:00');
});
ok('每天 · 空槽位被过滤，不产生尾随斜杠', () => {
  S.drafts = daily('09:00', '', '   ');
  assert.equal(remCompose('rem').value, '09:00');
});
ok('每天 · 一个时刻都没选 → 短句报错且不教手输格式', () => {
  S.drafts = daily('', '');
  const r = remCompose('rem');
  assert.equal(r.value, undefined);
  assert.match(r.err, /先选一个提醒时刻/);
  assert.doesNotMatch(r.err, /HH:MM|YYYY/);
});
ok('单次 · 有日期 → YYYY-MM-DDTHH:mm', () => {
  S.drafts = { remMode: 'once', remTimes: ['08:30'], remDate: '2026-09-05' };
  assert.equal(remCompose('rem').value, '2026-09-05T08:30');
});
ok('单次 · 缺日期 → 明确指向「每天」这条退路', () => {
  S.drafts = { remMode: 'once', remTimes: ['08:30'], remDate: '' };
  assert.match(remCompose('rem').err, /选日期/);
});
ok('单次 · 塞了多个时刻 → 拒绝而不是悄悄丢一个', () => {
  S.drafts = { remMode: 'once', remTimes: ['08:30', '20:00'], remDate: '2026-09-05' };
  assert.match(remCompose('rem').err, /只能一个时刻/);
});

console.log('qa-3 · v1 遗留多时刻 round-trip（进编辑不改值直接存，必须原样回去）');
for (const t of ['09:00/14:00', '09:00/10:00/14:00/16:00', '09:00', '2026-09-05T08:30']) {
  ok(`time="${t}" 全量回填后重组仍等于原值`, () => {
    S.reminders = [{ id: 'r1', time: t, title: 'x' }];
    S.drafts = { remMode: 'daily', remTimes: [''], remEditMode: 'daily', remEditTimes: [''], remEditDate: '' };
    S.errs = {};
    act['rem-edit']({ dataset: { id: 'r1' } });
    assert.equal(S.remEdit, 'r1');
    const c = remCompose('remEdit');
    assert.equal(c.err, undefined, `回填后组装失败：${c.err}`);
    assert.equal(c.value, t);
    if (t.includes('/')) {
      assert.equal(S.drafts.remEditTimes.length, t.split('/').length, '多时刻必须展开成等量槽位');
      assert.equal(S.drafts.remEditMode, 'daily');
    }
    if (t.includes('T')) {
      assert.equal(S.drafts.remEditMode, 'once');
      assert.equal(S.drafts.remEditDate, '2026-09-05');
    }
  });
}

console.log('Ask 1 · 模式切换的收敛行为');
ok('每天(3 个时刻) → 单次：只留第一个，不留隐藏多值', () => {
  S.drafts = daily('09:00', '14:00', '20:00');
  setRemMode('rem', 'once');
  assert.equal(S.drafts.remMode, 'once');
  assert.deepEqual(S.drafts.remTimes, ['09:00']);
});
ok('单次 → 每天：不吞掉已选时刻', () => {
  S.drafts = { remMode: 'once', remTimes: ['08:30'], remDate: '2026-09-05' };
  setRemMode('rem', 'daily');
  assert.deepEqual(S.drafts.remTimes, ['08:30']);
  assert.equal(S.drafts.remMode, 'daily');
});

console.log('remTimeEditor · 渲染契约');
ok('每天模式：出「每天/单次」开关与 +时刻，不出日期框', () => {
  S.drafts = daily('09:00'); S.errs = {};
  const h = remTimeEditor('rem');
  assert.match(h, /data-act="rem-mode" data-arg="daily"/);
  assert.match(h, /data-act="rem-mode" data-arg="once"/);
  assert.match(h, /\+ 时刻/);
  assert.doesNotMatch(h, /type="date"/);
});
ok('单次模式：出日期框，收掉 +时刻', () => {
  S.drafts = { remMode: 'once', remTimes: ['08:30'], remDate: '2026-09-05' }; S.errs = {};
  const h = remTimeEditor('rem');
  assert.match(h, /type="date"/);
  assert.doesNotMatch(h, /\+ 时刻/);
});
ok('多槽位：每个都可删（删空会自动补一个空槽，删不掉最后一个）', () => {
  S.drafts = daily('09:00', '14:00'); S.errs = {};
  const h = remTimeEditor('rem');
  assert.equal((h.match(/data-act="rem-time-del"/g) || []).length, 2);
});
ok('单槽位：不给删除按钮', () => {
  S.drafts = daily('09:00'); S.errs = {};
  const h = remTimeEditor('rem');
  assert.equal((h.match(/data-act="rem-time-del"/g) || []).length, 0);
});
ok('每个控件都有 aria-label（可访问性基线）', () => {
  S.drafts = daily('09:00', '14:00'); S.errs = {};
  const h = remTimeEditor('rem');
  assert.match(h, /aria-label="提醒时刻 1"/);
  assert.match(h, /aria-label="提醒时刻 2"/);
  assert.match(h, /aria-label="删掉第 2 个时刻"/);
});

console.log(`\n${pass} 项全绿`);
