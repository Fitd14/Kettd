import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

/**
 * 托盘菜单契约门禁（sticky-shell-polish S1 · 1.7）。
 *
 * 契约来源：openspec/changes/sticky-shell-polish/specs/app-shell/tray/spec.md
 * - 托盘只允许五项：打开主界面 / 待办悬浮窗（勾选态）/ 新建便签 / 打开数据文件夹 / 退出；
 * - 快速记录、显示全部便签、隐藏全部便签、管理提醒、本周汇总导出 MUST NOT 再出现在托盘；
 * - 文档（ARCHITECTURE §5 / README 托盘菜单段）必须与实现一致。
 *
 * Rust 的 SystemTrayMenu 无法从 JS 直接读取，与 command-parity / main-window-close 同一手法：
 * 静态锁住 runtime.rs 的源码形状，任何人把被删项加回来或改坏分组当场红。
 */
const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const read = (p) => readFileSync(join(root, p), 'utf8');

const runtimeRs = read('src-tauri/src/runtime.rs');
const architecture = read('docs/ARCHITECTURE.md');
const readme = read('README.md');

const KEPT = [
  ['open_main', '打开主界面'],
  ['todo_float', '待办悬浮窗'],
  ['sticky_new', '新建便签'],
  ['data_dir', '打开数据文件夹'],
  ['quit', '退出'],
];
const REMOVED_IDS = ['capture', 'sticky_show_all', 'sticky_hide_all', 'reminders', 'weekly'];
const REMOVED_LABELS = ['快速记录', '显示全部便签', '隐藏全部便签', '管理提醒', '本周汇总导出'];

/** 取模块级函数体：从签名行到首个顶格 `}`（本仓库 fn 一律 0 缩进） */
function fnBody(source, signature) {
  const start = source.indexOf(signature);
  assert.notEqual(start, -1, `找不到函数：${signature}`);
  const rest = source.slice(start);
  const end = rest.search(/^\}/m);
  assert.notEqual(end, -1, `函数体未闭合：${signature}`);
  return rest.slice(0, end);
}

test('托盘菜单只有五项，顺序与文案固定', () => {
  const menu = fnBody(runtimeRs, 'pub fn tray_menu');

  // 链式 .add_item(...) 的顺序 = 托盘实际顺序；todo_item 是勾选项变量
  const added = [...menu.matchAll(/\.add_item\((?:CustomMenuItem::new\("([a-z_]+)", "([^"]+)"\)|(todo_item))\)/g)].map((m) =>
    m[1] ? [m[1], m[2]] : ['todo_float', '待办悬浮窗'],
  );
  assert.deepEqual(
    added,
    KEPT,
    `托盘菜单应为 ${KEPT.map(([id]) => id).join(' / ')}，实际 ${added.map(([id]) => id).join(' / ')}`,
  );
  assert.equal((menu.match(/\.add_item\(/g) || []).length, 5, '托盘可点项必须恰好 5 个');

  // 分组：主界面/悬浮窗 · 便签/数据文件夹 · 退出，共 2 条分隔线
  assert.equal(
    (menu.match(/\.add_native_item\(SystemTrayMenuItem::Separator\)/g) || []).length,
    2,
    '托盘分组分隔线必须恰好 2 条',
  );

  // 待办悬浮窗保留勾选态
  assert.match(menu, /CustomMenuItem::new\("todo_float", "待办悬浮窗"\)/, '找不到待办悬浮窗菜单项');
  assert.match(menu, /if todo_visible \{[\s\S]*?\.selected\(\)/, '待办悬浮窗必须按可见状态渲染勾选态');
});

test('被删的五个托盘子项不得回流', () => {
  const menu = fnBody(runtimeRs, 'pub fn tray_menu');
  for (const id of REMOVED_IDS) {
    assert.doesNotMatch(menu, new RegExp(`"${id}"`), `托盘菜单又出现已删项 "${id}"`);
  }
  for (const label of REMOVED_LABELS) {
    assert.doesNotMatch(menu, new RegExp(label), `托盘菜单又出现已删文案「${label}」`);
  }
});

test('托盘点击分发只认保留的五项，且主窗失败可见', () => {
  const body = fnBody(runtimeRs, 'pub fn handle_tray_click');
  const arms = [...body.matchAll(/^\s{4}"([a-z_]+)" =>/gm)].map((m) => m[1]);
  // match 分支顺序不影响行为，只锁「恰好这五项」
  assert.deepEqual(
    [...arms].sort(),
    KEPT.map(([id]) => id).sort(),
    `托盘分发分支应只有 ${KEPT.map(([id]) => id).join(' / ')}，实际 ${arms.join(' / ')}`,
  );

  for (const id of REMOVED_IDS) {
    assert.doesNotMatch(body, new RegExp(`"${id}"`), `托盘分发仍留着已删分支 "${id}"`);
  }
  assert.match(body, /"open_main" => \{\s*report_window_error\(app, show_main\(app, None\)\);/, '打开主界面必须走可见上报');
  assert.doesNotMatch(body, /let _ = show_main/, '托盘打开主窗不得吞掉失败');
  assert.match(body, /"quit" => return true,/, '退出项必须返回 true 交给上层退出');

  // 依赖被删分支的帮手函数应一并删除；仍在别处使用的窗口帮手必须保留
  assert.doesNotMatch(runtimeRs, /fn note_snapshots/, 'note_snapshots 只服务已删的便签显隐项，应删除');
  assert.match(runtimeRs, /fn open_note_window/, 'open_note_window 仍被 commands/main 使用，不得删除');
  assert.match(runtimeRs, /fn hide_note_window/, 'hide_note_window 仍被 commands 使用，不得删除');
});

test('ARCHITECTURE §5 托盘段落与实现一致', () => {
  const menuLine = architecture
    .split('\n')
    .find((line) => line.includes('`打开主界面`') && line.includes('· 分隔'));
  assert.ok(menuLine, 'ARCHITECTURE.md §5 找不到托盘菜单段');
  for (const [, label] of KEPT) {
    assert.ok(menuLine.includes(label), `ARCHITECTURE 托盘段缺「${label}」`);
  }
  for (const label of REMOVED_LABELS) {
    assert.ok(!menuLine.includes(label), `ARCHITECTURE 托盘段仍把「${label}」写成现行菜单项`);
  }
});

test('README 托盘菜单段与实现一致', () => {
  const section = readme.match(/### 托盘菜单\r?\n\r?\n([^\r\n]+)/);
  assert.ok(section, 'README 找不到「托盘菜单」段');
  const line = section[1];
  for (const [, label] of KEPT) {
    assert.ok(line.includes(label), `README 托盘菜单缺「${label}」`);
  }
  for (const label of REMOVED_LABELS) {
    assert.ok(!line.includes(label), `README 托盘菜单仍列着已删项「${label}」`);
  }
});