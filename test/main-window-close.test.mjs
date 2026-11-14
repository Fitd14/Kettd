import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

/**
 * 主窗关闭路径门禁。
 *
 * 背景（2026-11-09 真机取证）：主窗 decorations:false，✕ 由前端 appWindow.close() 实现；
 * tauri 1.8.3 的 close 走 tauri-runtime-wry `on_window_close`（丢掉 tao Window → DestroyWindow），
 * 全程不产生 CloseRequested，main.rs 里的 api.prevent_close() 无从拦截。主窗一旦被销毁，
 * show_main 的 get_window("main") 拿不到窗口、错误又被 `let _ =` 吞掉，于是托盘「打开主界面」
 * 彻底失效；进程仍在 → 单实例互斥体还持着 → 之后每次启动都被拒。表现为「第一次能开，后续全打不开」。
 *
 * 与 command-parity 同一手法：静态锁住源码形状，任何人改回销毁语义当场红。
 */
const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const read = (p) => readFileSync(join(root, p), 'utf8');

const titleBar = read('src-react/src/components/title-bar.tsx');
const apiTs = read('src-react/src/lib/api.ts');
const commandsRs = read('src-tauri/src/commands.rs');
const runtimeRs = read('src-tauri/src/runtime.rs');

/** 取模块级函数体：从签名行到首个顶格 `}`（本仓库 fn 一律 0 缩进） */
function fnBody(source, signature) {
  const start = source.indexOf(signature);
  assert.notEqual(start, -1, `找不到函数：${signature}`);
  const rest = source.slice(start);
  const end = rest.search(/^\}/m);
  assert.notEqual(end, -1, `函数体未闭合：${signature}`);
  return rest.slice(0, end);
}

test('主窗 ✕ 只隐藏，不得调用销毁窗口的 close()', () => {
  const closeButton = titleBar.match(/title="关闭"[\s\S]*?<\/button>/);
  assert.ok(closeButton, '标题栏找不到关闭按钮');
  assert.doesNotMatch(closeButton[0], /close\?\.\(\)/, '✕ 调用了 appWindow.close()，会销毁主窗（见文件头说明）');
  assert.match(closeButton[0], /hideMainWindow\(\)/, '✕ 必须改走 hide_main_window 命令');
});

test('api.ts 封装 hide_main_window', () => {
  assert.match(apiTs, /export const hideMainWindow = \(\) => call<void>\('hide_main_window'\)/);
});

test('后端 hide_main_window 用隐藏语义，不碰 close/destroy', () => {
  const body = fnBody(commandsRs, 'pub fn hide_main_window');
  assert.match(body, /runtime::hide_window\(&app, runtime::MAIN_LABEL\)/);
  assert.doesNotMatch(body, /\.close\(\)|\.destroy\(\)/, '隐藏命令里出现销毁调用');
});

test('主窗缺失时 show_main 必须重建，而不是报错返回', () => {
  const body = fnBody(runtimeRs, 'pub fn show_main');
  assert.doesNotMatch(body, /window_of\(app, MAIN_LABEL\)\?/, 'show_main 仍在缺窗时直接早退');
  assert.match(body, /restore_main_window\(app, route\)/, 'show_main 必须经缺窗重建兜底');

  const restore = fnBody(runtimeRs, 'fn restore_main_window');
  assert.match(restore, /MAIN_LABEL/, '重建窗口必须用 main 标签');
  assert.match(restore, /build_with_timeout/, '重建必须走统一超时包装（WebView2 建窗挂起防线）');
});

test('托盘打开主窗失败必须可见，不再 let _ = 吞掉', () => {
  const body = fnBody(runtimeRs, 'pub fn handle_tray_click');
  assert.doesNotMatch(body, /let _ = show_main/, '托盘分发仍吞掉 show_main 的错误');
  assert.match(body, /report_window_error\(app, show_main/, '主窗打开失败必须走可见上报');

  const report = fnBody(runtimeRs, 'fn report_window_error');
  assert.match(report, /notify_title\(/, '上报最终要落到系统通知');

  const hotkeyArm = runtimeRs.match(/HotkeyAction::OpenMain => \{[\s\S]*?\n    \}/);
  assert.ok(hotkeyArm, '找不到 OpenMain 热键分支');
  assert.doesNotMatch(hotkeyArm[0], /let _ = show_main/, '热键打开主窗仍在吞掉失败');
});
