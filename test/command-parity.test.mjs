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
const doc = read('src-tauri/docs/V2-API.md');

// ① generate_handler![] 里注册的命令名
const handlerBody = mainRs.match(/generate_handler!\[([\s\S]*?)\]/)[1];
const handler = new Set([...handlerBody.matchAll(/commands::([a-z_]+)/g)].map((m) => m[1]));

// ② #[tauri::command] 函数名（含 async fn —— 建窗类命令必须 async，真机复盘 2026-09-08）
const defined = new Set([...commandsRs.matchAll(/#\[tauri::command\]\s*(?:pub\s+)?(?:async\s+)?fn\s+([a-z_]+)/g)].map((m) => m[1]));

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

test('命令总数与文档声明一致（当前 53）', () => {
  assert.equal(handler.size, 53, '总数变更时同步更新 V2-API §10 与本断言');
});
