#!/usr/bin/env node
/**
 * 版本单一来源工具。
 *
 * 产品版本在四个清单里各存一份，人工改必然漂移（历史上根 package.json 停在 0.0.1，
 * 与 2.2.0 的安装包对不上）。这里把「读 / 校验 / 写」收敛到一个入口，打包脚本与门禁都走它。
 *
 *   node scripts/version.mjs get            打印当前版本（以 tauri.conf.json 为准）
 *   node scripts/version.mjs check          校验四处一致；不一致退出码 1
 *   node scripts/version.mjs set 2.3.0      把版本写入四处并回显
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

/** @type {Array<{label: string, path: string, kind: 'json'|'toml', path_in_json?: string[], read: (text: string) => string, write: (text: string, v: string) => string}>} */
const TARGETS = [
  {
    label: 'package.json',
    path: 'package.json',
    kind: 'json',
    read: (text) => JSON.parse(text).version,
    // 只替换第一个 "version" 键，保留原文件缩进与键序，避免整文件重排
    write: (text, v) => text.replace(/("version"\s*:\s*")[^"]*(")/, `$1${v}$2`),
  },
  {
    label: 'src-react/package.json',
    path: 'src-react/package.json',
    kind: 'json',
    read: (text) => JSON.parse(text).version,
    write: (text, v) => text.replace(/("version"\s*:\s*")[^"]*(")/, `$1${v}$2`),
  },
  {
    label: 'src-tauri/tauri.conf.json',
    path: 'src-tauri/tauri.conf.json',
    kind: 'json',
    read: (text) => JSON.parse(text).package.version,
    // 版本嵌在 package 块里，正则限定在该块内替换，避免误伤 tauri 块中的同名字段
    write: (text, v) =>
      text.replace(/("package"\s*:\s*\{[\s\S]*?"version"\s*:\s*")[^"]*(")/, `$1${v}$2`),
  },
  {
    label: 'src-tauri/Cargo.toml',
    path: 'src-tauri/Cargo.toml',
    kind: 'toml',
    read: (text) => {
      const pkg = text.split(/^\[/m).find((s) => s.startsWith('package]'));
      const m = pkg && pkg.match(/^version\s*=\s*"([^"]+)"/m);
      if (!m) throw new Error('Cargo.toml 找不到 [package] version');
      return m[1];
    },
    write: (text, v) => text.replace(/^version\s*=\s*"[^"]*"/m, `version = "${v}"`),
  },
];

const VERSION_RE = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;

const load = (t) => readFileSync(join(root, t.path), 'utf8');

function readAll() {
  return TARGETS.map((t) => ({ target: t, version: t.read(load(t)) }));
}

function cmdGet() {
  const main = TARGETS[2];
  console.log(main.read(load(main)));
}

function cmdCheck() {
  const rows = readAll();
  const width = Math.max(...rows.map((r) => r.target.label.length));
  const versions = new Set(rows.map((r) => r.version));
  console.log(`版本源：${TARGETS[2].path}`);
  for (const r of rows) {
    console.log(`  ${r.target.label.padEnd(width)}  ${r.version}`);
  }
  if (versions.size === 1) {
    console.log(`\nOK 四处一致：${[...versions][0]}`);
    return 0;
  }
  console.error('\nFAIL 版本不一致，执行 `node scripts/version.mjs set <x.y.z>` 统一');
  return 1;
}

function cmdSet(next) {
  if (!VERSION_RE.test(next)) {
    console.error(`版本号格式非法：${next}（期望 MAJOR.MINOR.PATCH，可带预发布后缀）`);
    return 1;
  }
  for (const t of TARGETS) {
    const before = load(t);
    const after = t.write(before, next);
    if (after === before && t.read(before) !== next) {
      throw new Error(`写入失败（未匹配到版本字段）：${t.path}`);
    }
    if (after !== before) writeFileSync(join(root, t.path), after);
    console.log(`  ${t.label.padEnd(24)} ${t.read(load(t))}`);
  }
  console.log(`\nOK 版本已统一为 ${next}（Cargo.lock 会在下次 cargo 构建时自动跟上）`);
  return 0;
}

const [cmd, arg] = process.argv.slice(2);
try {
  if (cmd === 'get') {
    cmdGet();
  } else if (cmd === 'check') {
    process.exitCode = cmdCheck();
  } else if (cmd === 'set' && arg) {
    process.exitCode = cmdSet(arg);
  } else {
    console.error('用法: node scripts/version.mjs <get|check|set <x.y.z>>');
    process.exitCode = 2;
  }
} catch (err) {
  console.error(`FAIL ${err.message}`);
  process.exitCode = 1;
}
