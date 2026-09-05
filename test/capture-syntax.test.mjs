/**
 * 快录语法解析 · 特征化测试（ADR-0006）
 *
 * 这批断言是在**把 parseCapture 从 api.js 搬到 kernel 之前**录下的现状行为，
 * 搬完直接跑：任何语义漂移都会在这里变红，而不是等真机用户发现。
 * 冻结包含缺陷在内的现有行为（见 KNOWN DEFECT 标注）—— 特征化测试的职责是
 * "证明改动没引入差异"，不是"证明现状正确"；修缺陷时请同步改这条断言。
 *
 * 跑法：node test/capture-syntax.test.mjs
 */
import assert from 'node:assert/strict';
import { parseCapture, buildCaptureArgs } from '../src/kernel/capture.js';

const TODAY = '2026-09-05'; // 周六
let pass = 0;
const shape = (r) => ({
  title: r.title,
  dueAt: r.dueAt ?? null,
  auto: !!r.autoRemind,
  cat: r.category ?? null,
  pri: r.priority ?? null,
  fb: !!r.fellBack,
  chips: r.chips.map((c) => `${c.kind}:${c.value}`),
});
const caseOf = (input, expect) => {
  it(input, () => assert.deepEqual(shape(parseCapture(input, TODAY)), expect));
};
const it = (name, fn) => { fn(); pass++; console.log('  ✓', name); };

console.log('语法解析 · 分类/优先级 sigil');
caseOf('买牛奶', { title: '买牛奶', dueAt: null, auto: false, cat: null, pri: null, fb: true, chips: [] });
caseOf('取快递', { title: '取快递', dueAt: null, auto: false, cat: null, pri: null, fb: true, chips: [] });
caseOf('#工作', { title: '#工作', dueAt: null, auto: false, cat: '工作', pri: null, fb: false, chips: ['category:分类 · 工作'] });
caseOf('!高', { title: '!高', dueAt: null, auto: false, cat: null, pri: 'high', fb: false, chips: ['priority:优先级 · 高'] });
caseOf('明天下午3点 #乱分类 !乱优先级', {
  title: '#乱分类 !乱优先级', dueAt: '2026-09-06T15:00', auto: true, cat: null, pri: null,
  fb: false, chips: ['time:时间 · 15:00', 'date:日期 · 2026-09-06'],
});

console.log('语法解析 · 相对日期');
caseOf('明天', { title: '明天', dueAt: '2026-09-06', auto: true, cat: null, pri: null, fb: false, chips: ['date:日期 · 2026-09-06'] });
caseOf('后天 #生活 低', { title: '低', dueAt: '2026-09-07', auto: true, cat: '生活', pri: null, fb: false, chips: ['category:分类 · 生活', 'date:日期 · 2026-09-07'] });
caseOf('周五上午10点开会', { title: '开会', dueAt: '2026-09-11T10:00', auto: true, cat: null, pri: null, fb: false, chips: ['time:时间 · 10:00', 'date:日期 · 2026-09-11'] });
caseOf('下周一体检', { title: '体检', dueAt: '2026-09-14', auto: true, cat: null, pri: null, fb: false, chips: ['date:日期 · 2026-09-14'] });

console.log('语法解析 · 绝对日期与时段');
caseOf('9月10日 晚上8点 读书', { title: '读书', dueAt: '2026-09-10T20:00', auto: true, cat: null, pri: null, fb: false, chips: ['time:时间 · 20:00', 'date:日期 · 2026-09-10'] });
caseOf('9.10 提交材料', { title: '提交材料', dueAt: '2026-09-10', auto: true, cat: null, pri: null, fb: false, chips: ['date:日期 · 2026-09-10'] });
caseOf('晚上11点提醒喝水', { title: '提醒喝水', dueAt: '2026-09-05T23:00', auto: true, cat: null, pri: null, fb: false, chips: ['time:时间 · 23:00'] });
caseOf('今天 18:00 交周报 #工作', { title: '交周报', dueAt: '2026-09-05T18:00', auto: true, cat: '工作', pri: null, fb: false, chips: ['category:分类 · 工作', 'time:时间 · 18:00', 'date:日期 · 2026-09-05'] });
caseOf('明天早上9点背单词 #学习 !中', { title: '背单词', dueAt: '2026-09-06T09:00', auto: true, cat: '学习', pri: 'med', fb: false, chips: ['category:分类 · 学习', 'priority:优先级 · 中', 'time:时间 · 09:00', 'date:日期 · 2026-09-06'] });

console.log('语法解析 · 兜底与空输入');
caseOf('  ', { title: '', dueAt: null, auto: false, cat: null, pri: null, fb: true, chips: [] });
caseOf('', { title: '', dueAt: null, auto: false, cat: null, pri: null, fb: true, chips: [] });
caseOf('下午两点半开会', { title: '下午两点半开会', dueAt: null, auto: false, cat: null, pri: null, fb: true, chips: [] });

console.log('已知缺陷（原样冻结，修它时同步改这条）');
it('ISO 串会被拆坏：日期丢失、dueAt 静默落到今天', () => {
  const r = shape(parseCapture('2026-09-20T14:30 复盘', TODAY));
  assert.equal(r.title, '2026-09-20T 复盘', '标题残留了被切一半的日期');
  assert.equal(r.dueAt, '2026-09-05T14:30', '期望 09-20 却拿到 09-05 —— 错日期且不报错，比解析失败更糟');
});

console.log('buildCaptureArgs · 三份复制收敛后的唯一实现');
it('有截止：dueAt 与 remindAt 同步带上', () => {
  const args = buildCaptureArgs(parseCapture('明天下午3点找导师 #学习 !高', TODAY));
  assert.deepEqual(args, {
    title: '找导师', source: 'capture', category: '学习', priority: 'high',
    dueAt: '2026-09-06T15:00', remindAt: '2026-09-06T15:00',
  });
});
it('无截止：只带标题与兜底口径，不出现 dueAt/remindAt 键', () => {
  assert.deepEqual(buildCaptureArgs(parseCapture('取快递', TODAY)),
    { title: '取快递', source: 'capture', category: '生活', priority: 'med' });
});
it('source 可覆盖（主窗手动添加走 manual）', () => {
  assert.equal(buildCaptureArgs(parseCapture('取快递', TODAY), 'manual').source, 'manual');
});

console.log(`\n${pass} 项全绿`);
