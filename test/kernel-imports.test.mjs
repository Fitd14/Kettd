/**
 * 共享内核导入一致性检查（ADR-0006）
 *
 * 为什么需要：vanilla 三窗**没有构建步骤**，`import { A, B } from './kernel/x.js'`
 * 里写错一个名字，浏览器只会在运行时抛 ReferenceError —— 而运行时表现是"整个
 * module script 崩掉、页面全白"，没有编译期报错、没有类型检查。
 * 本检查在 node 里静态比对「导入名集合 ⊆ 内核导出名集合」，把这类错误挡在提交前。
 *
 * 跑法：node test/kernel-imports.test.mjs
 */
import assert from 'node:assert/strict';
import { readFileSync, readdirSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const KDIR = path.join(root, 'src', 'kernel');

const exportsOf = (file) => {
  const src = readFileSync(file, 'utf8');
  const names = new Set();
  for (const m of src.matchAll(/export\s+(?:async\s+)?(?:function|const|let|class)\s+(\w+)/g)) names.add(m[1]);
  for (const m of src.matchAll(/export\s*\{([^}]*)\}/g)) {
    for (const part of m[1].split(',')) {
      const n = part.trim().split(/\s+as\s+/).pop().trim();
      if (n) names.add(n);
    }
  }
  return names;
};

const kernelFiles = existsSync(KDIR) ? readdirSync(KDIR).filter((f) => f.endsWith('.js')) : [];
assert.ok(kernelFiles.length >= 2, `src/kernel/ 至少应有 text.js 与 rem-editor.js，实际：${kernelFiles.join(',')}`);

const exported = new Map();
for (const f of kernelFiles) exported.set(`./kernel/${f}`, exportsOf(path.join(KDIR, f)));

const consumers = ['src/index.html', 'src/float.html', 'src/capture.html', 'src/api.js']
  .filter((p) => existsSync(path.join(root, p)));
assert.ok(consumers.length >= 4, '消费方清单与实际文件不符，检查是否漏了窗口文件');

let checks = 0;
for (const rel of consumers) {
  const src = readFileSync(path.join(root, rel), 'utf8');
  for (const m of src.matchAll(/import\s*\{([^}]*)\}\s*from\s*['"](\.\/kernel\/[\w.-]+\.js)['"]/g)) {
    const spec = m[2];
    assert.ok(exported.has(spec), `${rel}: 引用了不存在的内核模块 ${spec}`);
    const names = m[1].split(',').map((s) => s.trim().split(/\s+as\s+/)[0]).filter(Boolean);
    for (const n of names) {
      assert.ok(exported.get(spec).has(n),
        `${rel} 从 ${spec} 导入 "${n}"，但该模块未导出它（vanilla 无构建步骤，这会在运行时白屏）`);
      checks++;
    }
  }
}
assert.ok(checks >= 10, `只比对了 ${checks} 个导入名，疑似正则没匹配上，检查内核引用写法`);

// 反向：内核不许留无人使用的导出（投机 API 会变第二份漂移实现）
const usedNames = new Set();
for (const rel of consumers) {
  const src = readFileSync(path.join(root, rel), 'utf8');
  for (const m of src.matchAll(/import\s*\{([^}]*)\}\s*from\s*['"]\.\/kernel\/[\w.-]+\.js['"]/g)) {
    for (const part of m[1].split(',')) {
      const n = part.trim().split(/\s+as\s+/)[0].trim();
      if (n) usedNames.add(n);
    }
  }
}
const unused = [...exported.values()].flatMap((set) => [...set]).filter((n) => !usedNames.has(n) && n !== 'esc');
assert.deepEqual(unused, [], `内核有无人使用的导出：${unused.join(', ')} —— 要么接上调用方，要么删掉`);

console.log(`  ✓ ${consumers.length} 个消费方、${checks} 个导入名全部可在内核中解析，且无未使用导出`);
console.log('\n导入一致性通过');
