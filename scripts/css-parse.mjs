#!/usr/bin/env node
/**
 * CSS 解析门禁：用 lightningcss（与 vite:css-post 同一条压缩链路）逐张解析
 * src-react/src 下所有 .css，提前拦下「tsc / eslint 都不看、只有构建才炸」的语法错。
 *
 * 为什么需要：2026-11-16 一次手工改样式把字面量 `n 写进了 kb.css，
 * tsc 与 eslint 全绿，`npm run build` 在 lightningcss 压缩阶段才报
 * 「Invalid empty selector」，一整轮发版被打回。
 *
 * 用法：
 *   node scripts/css-parse.mjs            # 扫 src-react/src 下全部 .css
 *   node scripts/css-parse.mjs a.css b.css # 只检查指定文件
 * 退出码：0 全部通过（或环境缺 lightningcss → 打印 [SKIP]，门禁记为 SKIP）；
 *         1 有文件解析失败。
 */
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.dirname(here);
const srcDir = path.join(root, 'src-react', 'src');

function loadLightningcss() {
  const requireFromFront = createRequire(path.join(root, 'src-react', 'package.json'));
  try {
    return requireFromFront('lightningcss');
  } catch {
    return null;
  }
}

function walk(dir, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name === 'node_modules' || entry.name.startsWith('.')) continue;
      walk(full, out);
    } else if (entry.name.endsWith('.css')) {
      out.push(full);
    }
  }
  return out;
}

const lightningcss = loadLightningcss();
if (!lightningcss) {
  console.log('[SKIP] 找不到 lightningcss（src-react/node_modules），CSS 解析门禁未执行');
  process.exit(0);
}

const args = process.argv.slice(2);
const files = args.length > 0
  ? args.map((f) => path.resolve(f))
  : walk(srcDir).sort();

if (files.length === 0) {
  console.log('[SKIP] src-react/src 下没有 .css 文件');
  process.exit(0);
}

let failed = 0;
for (const file of files) {
  const rel = path.relative(root, file).replace(/\\/g, '/');
  let code;
  try {
    code = fs.readFileSync(file);
  } catch (err) {
    failed += 1;
    console.log(`FAIL ${rel}  读取失败：${err.message}`);
    continue;
  }
  try {
    const out = lightningcss.transform({ filename: rel, code, minify: true });
    console.log(`OK   ${rel}  in=${code.length}  out=${out.code.length}`);
  } catch (err) {
    failed += 1;
    const where = err.loc ? ` (${err.loc.line}:${err.loc.column})` : '';
    console.log(`FAIL ${rel}${where}  ${err.message}`);
  }
}

console.log(`--- ${files.length} 张样式表，${failed} 张解析失败`);
process.exit(failed > 0 ? 1 : 0);