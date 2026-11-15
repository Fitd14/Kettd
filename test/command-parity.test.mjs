import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

/** 命令对账（V2-API §10 的常驻化）：generate_handler ↔ #[tauri::command] ↔ api.ts ↔ 文档表
 *  任何一侧新增/删除命令而其他侧没跟上，测试当场红 —— 不再依赖人肉跑脚本。 */
const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const read = (p) => readFileSync(join(root, p), 'utf8');

const mainRs = read('src-tauri/src/main.rs');
const commandsRs = read('src-tauri/src/commands.rs');
const apiTs = read('src-react/src/lib/api.ts');
const readingFontTs = read('src-react/src/lib/reading-font.ts');
const doc = read('docs/ARCHITECTURE.md');

// ① generate_handler![] 里注册的命令名
const handlerBody = mainRs.match(/generate_handler!\[([\s\S]*?)\]/)[1];
const handler = new Set([...handlerBody.matchAll(/(?:[a-z_]+::)*commands::([a-z_]+)/g)].map((m) => m[1]));

// ② #[tauri::command] 函数名（含 async fn —— 建窗类命令必须 async，真机复盘 2026-09-08）
const defined = new Set([
  ...commandsRs.matchAll(/#\[tauri::command\]\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-z_]+)/g),
].map((m) => m[1]));

// ③ api.ts 里 call('xxx') 的命令名
const wrapped = new Set([...apiTs.matchAll(/call(?:<[^>]*>)?\('([a-z_]+)'/g)].map((m) => m[1]));

// ④ 文档 §2 表格的命令名
const docTable = doc.slice(doc.indexOf('## 2.'), doc.indexOf('### patch'));
const documented = new Set([...docTable.matchAll(/^\|\s*`([a-z_]+)`/gm)].map((m) => m[1]));

const diff = (a, b) => [...a].filter((x) => !b.has(x));

test('后端定义 ↔ 注册表一一对应', () => {
  assert.deepEqual(diff(defined, handler), [], '定义了但没注册');
  assert.deepEqual(diff(handler, defined), [], '注册了但没定义');
});

test('React api.ts 覆盖全部已注册命令', () => {
  assert.deepEqual(diff(handler, wrapped), [], 'api.ts 缺封装');
});

test('api.ts 没有封装不存在的命令', () => {
  assert.deepEqual(diff(wrapped, handler), [], 'api.ts 封装了不存在的命令');
});

test('V2-API 文档表 ↔ 注册表一一对应', () => {
  assert.deepEqual(diff(handler, documented), [], '注册了但文档缺行');
  assert.deepEqual(diff(documented, handler), [], '文档有行但没注册');
});

test('命令总数与文档声明一致（当前 71）', () => {
  assert.equal(handler.size, 71, '总数变更时同步更新 V2-API §10 与本断言');
  assert.ok(handler.has('update_folder'), 'kb-file-tree 1.4：必须注册 update_folder（改名/移动/排序/置顶）');
  assert.ok(!handler.has('rename_folder'), 'kb-file-tree 1.4：rename_folder 已被 update_folder 取代，不得残留');
  assert.ok(handler.has('export_backup'), 'kb-storage-backup D3：必须注册手动导出命令');
  assert.ok(handler.has('import_backup'), 'kb-storage-backup D4：必须注册手动导入命令');
  assert.ok(handler.has('register_kb_hotkey'), 'kb-storage-backup S4：必须注册直达知识库热键命令');
});

// 阅读字体：Rust 白名单 ↔ 设置页选项，两处必须同键同序，否则用户选了后端不认的值
test('READING_FONT_SYSTEMS 两端一致（reading-font）', () => {
  const modelsRs = read('src-tauri/src/models.rs');
  const start = modelsRs.indexOf('pub const READING_FONT_SYSTEMS');
  assert.notEqual(start, -1, 'models.rs 必须定义 READING_FONT_SYSTEMS');
  const block = modelsRs.slice(start, modelsRs.indexOf('];', start));
  const rustKeys = [...block.matchAll(/\("([^"]+)",/g)].map((m) => m[1]);
  const tsKeys = [...readingFontTs.matchAll(/\{ key: '([^']+)', label: '[^']+' \}/g)].map((m) => m[1]);
  assert.ok(rustKeys.length >= 4, 'Rust 侧至少要有四个系统字族');
  assert.deepEqual(tsKeys, rustKeys, '前端 READING_FONT_SYSTEMS 的键与顺序必须与 models.rs 一致');
});
