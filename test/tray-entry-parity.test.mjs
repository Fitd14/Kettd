import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

/**
 * 托盘「移除项不丢失功能入口」门禁（sticky-shell-polish · tasks 1.10）。
 *
 * 契约来源：openspec/changes/sticky-shell-polish/specs/app-shell/tray/spec.md
 * 「被移除的托盘菜单项 MUST NOT 让对应功能失去入口：快速记录仍可由全局热键与设置页触发、
 *   便签显隐仍可由便签卡片页 / 便签窗完成、提醒与周汇总仍可在其一界面内打开。」
 *
 * tray-menu.test.mjs 只锁托盘菜单本身的形状（五项、无回流），不证明功能还有别处可进。
 * 本门禁补后半句：五个被删项各自的前端入口必须留在源码里，谁把入口一起删掉当场红。
 *
 * 与 command-parity / main-window-close 同一手法：Rust 与 React 都从 JS 侧读不到运行态，
 * 静态锁住入口所在的调用点，改动入口就先改这条门禁。
 */
const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const read = (p) => readFileSync(join(root, p), 'utf8');

const readme = read('README.md');

/** 功能入口：每项给出「功能名 + 入口所在文件 + 必须出现的调用点正则」 */
const ENTRIES = [
  {
    name: '快速记录',
    file: 'src-react/src/views/SettingsView.tsx',
    // 设置页「快速记录」行：热键输入 + 换绑按钮（rebind('capture')）
    patterns: [
      [/aria-label="快速记录热键"/, '设置页缺少快速记录热键输入框'],
      [/void rebind\('capture'\)/, '设置页缺少快速记录换绑按钮'],
    ],
  },
  {
    name: '便签：显示 / 隐藏全部',
    file: 'src-react/src/views/StickyView.tsx',
    // 卡片页每张便签的显示/隐藏动作走 updateSticky({visible})，隐藏态先置可见再打开
    patterns: [
      [/updateSticky\(\w+\.id, \{ visible: (true|false) \}\)/, '便签卡片页缺少显示 / 隐藏全部便签的写入口'],
      [/await openSticky\(/, '便签卡片页缺少打开便签窗口的入口'],
    ],
  },
  {
    name: '提醒：管理入口',
    file: 'src-react/src/views/PlannedView.tsx',
    // 计划页读出提醒列表并展示「下一条待响」，是托盘「管理提醒」被删后的唯一查看口
    patterns: [
      [/boot\.reminders/, '计划页不再读取 reminders，提醒失去查看入口'],
      [/nextReminderTime\(/, '计划页缺少下一条提醒的计算'],
    ],
  },
  {
    name: '本周汇总导出',
    file: 'src-react/src/views/ReviewView.tsx',
    // 回顾页「周汇总草稿」折叠块里的导出按钮
    patterns: [
      [/exportWeekly\(\{ week: 'current', format: 'md' \}\)/, '回顾页缺少导出本周汇总的调用'],
      [/生成并导出本周汇总/, '回顾页缺少导出本周汇总的按钮文案'],
    ],
  },
];

for (const entry of ENTRIES) {
  test(`托盘已删项「${entry.name}」仍有前端入口：${entry.file}`, () => {
    const source = read(entry.file);
    for (const [pattern, message] of entry.patterns) {
      assert.match(source, pattern, message);
    }
  });
}

test('被删项的功能入口在 README 有文档落点', () => {
  const anchors = [
    ['快速记录', () => readme.includes('Alt+Shift+A') || readme.includes('快速记录')],
    ['显示全部便签', () => readme.includes('便签')],
    ['管理提醒', () => readme.includes('提醒')],
    ['本周汇总导出', () => readme.includes('周汇总') || readme.includes('本周汇总')],
  ];
  for (const [label, check] of anchors) {
    assert.ok(check(), `README 找不到「${label}」的功能说明，托盘删除后用户无处可查`);
  }
});