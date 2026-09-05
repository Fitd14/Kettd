/**
 * 提醒时刻编辑器回归测试（v2.1 · T3 → ADR-0006 第二形态）
 *
 * 关键变化：不再从 src/index.html 抽源码文本求值，改为 **import 共享内核**。
 * 原因（ADR-0006 / 风险 F1）：抽源码文本的测试在 M3 把视图搬进 React 后会静默失效 ——
 * 文件路径、函数名、ACTS 表、innerHTML 契约全部消失，而失效方式是"抽不到就抛错"，
 * 极易被当成环境问题跳过。qa-3（v1 多时刻不得静默降级）的可证伪保证就挂在这里。
 *
 * 覆盖：qa-3 多时刻 round-trip、qa-2 文案不教手输、Ask 1 显式「每天/单次」开关的收敛、
 *       渲染契约与 aria-label、槽位增删边界、转义防线。
 *
 * 跑法：node test/rem-editor.test.mjs   （零依赖，非零退出码即失败）
 */
import assert from 'node:assert/strict';
import {
  parseRemTime, composeRemTime, nextTimesForMode, removeTimeSlot, appendTimeSlot,
  prettyRemTime, renderRemEditor, emptyRemDraft, normMode, REM_ERRORS,
} from '../src/kernel/rem-editor.js';

let pass = 0;
const ok = (name, fn) => { fn(); pass++; console.log('  ✓', name); };
const daily = (...times) => ({ mode: 'daily', date: '', times });
const once = (date, time) => ({ mode: 'once', date, times: [time] });

console.log('composeRemTime · 每天=多时刻串 / 单次=带日期');
ok('每天 · 两个时刻 → HH:MM/HH:MM', () => {
  assert.equal(composeRemTime(daily('09:00', '14:00')).value, '09:00/14:00');
});
ok('每天 · 空槽位被过滤，不产生尾随斜杠', () => {
  assert.equal(composeRemTime(daily('09:00', '', '   ')).value, '09:00');
});
ok('每天 · 一个时刻都没选 → 短句报错且不教手输格式', () => {
  const r = composeRemTime(daily('', ''));
  assert.equal(r.value, undefined);
  assert.equal(r.err, REM_ERRORS.noTime);
  assert.doesNotMatch(r.err, /HH:MM|YYYY/);
});
ok('单次 · 有日期 → YYYY-MM-DDTHH:mm', () => {
  assert.equal(composeRemTime(once('2026-09-05', '08:30')).value, '2026-09-05T08:30');
});
ok('单次 · 缺日期 → 明确指向「每天」这条退路', () => {
  assert.match(composeRemTime(once('', '08:30')).err, /选日期/);
});
ok('单次 · 塞了多个时刻 → 拒绝而不是悄悄丢一个', () => {
  assert.match(composeRemTime({ mode: 'once', date: '2026-09-05', times: ['08:30', '20:00'] }).err, /只能一个时刻/);
});
ok('times 非数组不炸（undefined / null）', () => {
  assert.equal(composeRemTime({ mode: 'daily', times: undefined }).err, REM_ERRORS.noTime);
  assert.equal(composeRemTime({ mode: 'daily', times: null }).err, REM_ERRORS.noTime);
});

console.log('qa-3 · v1 遗留多时刻 round-trip（进编辑不改值直接存，必须原样回去）');
for (const t of ['09:00/14:00', '09:00/10:00/14:00/16:00', '09:00', '23:59', '2026-09-05T08:30', '']) {
  ok(`time="${t}" ${t ? '解析后重组仍等于原值' : '安全报错'}`, () => {
    const parsed = parseRemTime(t);
    const c = composeRemTime(parsed);
    if (!t) { assert.equal(c.err, REM_ERRORS.noTime); return; }
    assert.equal(c.err, undefined, `回填后组装失败：${c.err}`);
    assert.equal(c.value, t);
    if (t.includes('/')) {
      assert.equal(parsed.times.length, t.split('/').length, '多时刻必须展开成等量槽位');
      assert.equal(parsed.mode, 'daily');
    }
    if (t.includes('T')) {
      assert.equal(parsed.mode, 'once');
      assert.equal(parsed.date, '2026-09-05');
    }
  });
}
ok('脏输入（段间带空格）被容忍并归一', () => {
  assert.equal(composeRemTime(parseRemTime(' 09:00 / 14:00 ')).value, '09:00/14:00');
});
ok('parseRemTime(null / undefined) 不炸', () => {
  assert.equal(parseRemTime(null).mode, 'daily');
  assert.equal(parseRemTime(undefined).times.length, 1);
});

console.log('Ask 1 · 模式切换的收敛行为');
ok('每天(3 个时刻) → 单次：只留第一个，不留隐藏多值', () => {
  assert.deepEqual(nextTimesForMode(['09:00', '14:00', '20:00'], 'once'), ['09:00']);
});
ok('单次 → 每天：不吞掉已选时刻', () => {
  assert.deepEqual(nextTimesForMode(['08:30'], 'daily'), ['08:30']);
});
ok('全空 → 单次：给一个空槽而不是空数组', () => {
  assert.deepEqual(nextTimesForMode(['', '  '], 'once'), ['']);
});
ok('收敛是纯函数，不改入参', () => {
  const src = ['09:00', '14:00'];
  nextTimesForMode(src, 'once');
  assert.deepEqual(src, ['09:00', '14:00']);
});
ok('normMode 只认 once，其余一律 daily', () => {
  assert.equal(normMode('once'), 'once');
  assert.equal(normMode('daily'), 'daily');
  assert.equal(normMode(undefined), 'daily');
  assert.equal(normMode('ONCE'), 'daily');
});

console.log('槽位增删');
ok('append 后至少 1 个槽位，且不改入参', () => {
  const src = ['09:00'];
  assert.deepEqual(appendTimeSlot(src), ['09:00', '']);
  assert.deepEqual(src, ['09:00']);
  assert.deepEqual(appendTimeSlot(undefined), ['']);
});
ok('remove 到空会自动补一个空槽（最后一个删不掉）', () => {
  assert.deepEqual(removeTimeSlot(['09:00', '14:00'], 0), ['14:00']);
  assert.deepEqual(removeTimeSlot(['09:00'], 0), ['']);
});

console.log('renderRemEditor · 渲染契约');
const base = { prefix: 'rem', date: '', invalid: false, compact: false };
ok('每天模式：出「每天/单次」开关与 +时刻，不出日期框', () => {
  const h = renderRemEditor({ ...base, mode: 'daily', times: ['09:00'] });
  assert.match(h, /data-act="rem-mode" data-arg="daily"/);
  assert.match(h, /data-act="rem-mode" data-arg="once"/);
  assert.match(h, /\+ 时刻/);
  assert.doesNotMatch(h, /type="date"/);
});
ok('单次模式：出日期框，收掉 +时刻', () => {
  const h = renderRemEditor({ ...base, mode: 'once', date: '2026-09-05', times: ['08:30'] });
  assert.match(h, /type="date"/);
  assert.doesNotMatch(h, /\+ 时刻/);
});
ok('多槽位：每个都可删（删空会自动补，删不掉最后一个）', () => {
  const h = renderRemEditor({ ...base, mode: 'daily', times: ['09:00', '14:00'] });
  assert.equal((h.match(/data-act="rem-time-del"/g) || []).length, 2);
});
ok('单槽位：不给删除按钮', () => {
  const h = renderRemEditor({ ...base, mode: 'daily', times: ['09:00'] });
  assert.equal((h.match(/data-act="rem-time-del"/g) || []).length, 0);
});
ok('invalid=true 时每个时刻输入都带 invalid 类', () => {
  const h = renderRemEditor({ ...base, mode: 'daily', times: ['09:00', '14:00'], invalid: true });
  assert.equal((h.match(/class="input mono invalid/g) || []).length, 2);
});
ok('行内编辑用 remEdit 前缀，与添加行互不串台', () => {
  const h = renderRemEditor({ ...base, prefix: 'remEdit', mode: 'daily', times: ['09:00'], compact: true });
  assert.match(h, /data-in="remEditTimes"/);
  assert.match(h, /data-act="remEdit-mode"/);
  assert.doesNotMatch(h, /data-in="remTimes"/);
});
ok('每个控件都有 aria-label（可访问性基线）', () => {
  const h = renderRemEditor({ ...base, mode: 'daily', times: ['09:00', '14:00'] });
  assert.match(h, /aria-label="提醒时刻 1"/);
  assert.match(h, /aria-label="提醒时刻 2"/);
  assert.match(h, /aria-label="删掉第 2 个时刻"/);
  assert.match(h, /aria-label="再加一个提醒时刻"/);
});
ok('值里的引号与尖括号被转义（innerHTML 注入防线）', () => {
  const h = renderRemEditor({ ...base, mode: 'once', date: '"><script>', times: ['<b>'] });
  assert.doesNotMatch(h, /<script/);
  assert.match(h, /&lt;|&quot;|&#39;/);
});

console.log('prettyRemTime / emptyRemDraft');
ok('多时刻说个数、单次只说时刻、纯时刻原样、空串安全', () => {
  assert.equal(prettyRemTime('09:00/14:00'), '2 个时刻');
  assert.equal(prettyRemTime('2026-09-05T08:30'), '08:30');
  assert.equal(prettyRemTime('09:00'), '09:00');
  assert.equal(prettyRemTime(''), '');
});
ok('emptyRemDraft 是 daily + 一个空槽', () => {
  const e = emptyRemDraft();
  assert.equal(e.mode, 'daily');
  assert.deepEqual(e.times, ['']);
  assert.equal(e.date, '');
});

console.log(`\n${pass} 项全绿`);
