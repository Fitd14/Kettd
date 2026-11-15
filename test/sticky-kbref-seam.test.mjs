import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

/**
 * 便签卡片页「知识来源标题」接缝门禁（sticky-shell-polish · tasks 2.11）。
 *
 * 背景：卡片页把 `sticky.kbRef` 当知识条目 id，去 `get_kb_items` 返回的列表里找标题
 * （`StickyView.tsx` 的 `kbTitles` Map）。这条接缝横跨三个 change：
 * - 本 change（sticky-shell-polish S2）写卡片页；
 * - kb-file-tree 在并行改知识条目的标识与排序；
 * - kb-storage-backup 把 `get_kb_items` 收成「缺省只回元数据」，并新增 `ids` 批量取正文的形态。
 * 任何一方改掉 `KbItem.id` 的返回或 kbRef 的存法，卡片页会静默把来源显示成「知识条目已删除」，
 * 而不是报错——这类静默漂移只有门禁能拦。本文件同时锁前端一侧与后端一侧的契约形状。
 *
 * 与 command-parity 同一手法：从 JS 侧读不到 Rust / React 运行态，静态锁源码形状。
 */
const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const read = (p) => readFileSync(join(root, p), 'utf8');

/** 取模块级函数体：从签名行到首个顶格 `}`（本仓库 fn 一律 0 缩进） */
function fnBody(source, signature) {
  const start = source.indexOf(signature);
  assert.notEqual(start, -1, `找不到函数：${signature}`);
  const rest = source.slice(start);
  const end = rest.search(/^\}/m);
  assert.notEqual(end, -1, `函数体未闭合：${signature}`);
  return rest.slice(0, end);
}

const stickyView = read('src-react/src/views/StickyView.tsx');
const apiTs = read('src-react/src/lib/api.ts');
const commandsRs = read('src-tauri/src/commands.rs');

test('卡片页用 get_kb_items 的列表建 kbRef → 标题映射', () => {
  assert.match(stickyView, /getKbItems\(\)/, '卡片页不再拉取知识条目列表，来源标题恒为缺失');
  assert.match(
    stickyView,
    /new Map\(r\.data\.map\(\(item\) => \[item\.id, item\.title\]\)\)/,
    'kbRef → 标题映射必须按知识条目 id 建 Map（改字段名时同步本门禁）',
  );
  assert.match(stickyView, /st\.kbRef \? kbTitles\.get\(st\.kbRef\)/, '卡片必须按 kbRef 取标题');
});

test('知识条目缺失与读取失败必须分开提示，不得一律标成已删除', () => {
  assert.match(stickyView, /'知识条目已删除'/, '缺少「条目已删除」提示');
  assert.match(stickyView, /'知识来源读取失败'/, '读取失败被误报成条目已删除（假空态）');
  assert.match(stickyView, /'知识来源读取中…'/, '缺少加载态提示');
  assert.match(stickyView, /kbStatus === 'ready'/, '缺少 ready 判定，失败态会冒充「已删除」');
});

test('后端 get_kb_items 缺省回全量元数据（id + title 必须有）', () => {
  const body = fnBody(commandsRs, 'pub fn get_kb_items');
  assert.match(body, /None => Ok\(store\.kb\.iter\(\)\.map\(kb_metadata_only\)/, '缺省分支不再回全量条目，卡片页拿不到标题');
  const metadata = fnBody(commandsRs, 'fn kb_metadata_only');
  assert.match(metadata, /body_md: String::new\(\)/, '元数据形态必须把正文置空（IPC 体积契约）');
  assert.match(metadata, /\.\.item\.clone\(\)/, '元数据形态必须保留 id / title 等其余字段');
});

test('粘贴建窗仍把 kbRef 作为顶层参数（嵌进 args 恒为 None）', () => {
  assert.match(
    apiTs,
    /createSticky = [\s\S]{0,200}?call<[^>]*>\('create_sticky', \{ kbRef/,
    'create_sticky 的 kbRef 必须是 invoke 顶层参数（AGENTS §2.5）',
  );
});