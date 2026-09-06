import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

/** R2（ADR-0001）：双栈令牌是复制而非生成关系 —— 集合相等断言拦住静默漂移。 */
const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const vanilla = readFileSync(join(root, 'src', 'styles.css'), 'utf8');
const react = readFileSync(join(root, 'src-react', 'src', 'index.css'), 'utf8');

/** 抽取「行首选择器块」里的 `--xxx: 值` 令牌名集合（不比值：值一致由评审保证，集合漂移才是事故）。
    注意必须锚定行首 —— React 侧的 @custom-variant dark (&:is(.dark *)) 也含 ".dark" 字样。 */
function tokensOf(css, selector) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const re = new RegExp(`^${escaped}\\s*\\{`, 'm')
  const m = re.exec(css)
  if (!m) throw new Error(`找不到选择器 ${selector}`)
  const open = m.index + m[0].length - 1
  let depth = 1;
  let end = open;
  while (depth > 0 && end < css.length) {
    end += 1;
    if (css[end] === '{') depth += 1;
    if (css[end] === '}') depth -= 1;
  }
  const block = css.slice(open, end);
  const names = new Set();
  for (const m of block.matchAll(/(--[a-zA-Z0-9-]+)\s*:/g)) names.add(m[1]);
  return names;
}

test('亮色令牌集合：styles.css :root 与 index.css :root 完全相等', () => {
  const a = tokensOf(vanilla, ':root');
  const b = tokensOf(react, ':root');
  const onlyVanilla = [...a].filter((x) => !b.has(x));
  const onlyReact = [...b].filter((x) => !a.has(x));
  assert.deepEqual(
    { onlyVanilla, onlyReact },
    { onlyVanilla: [], onlyReact: [] },
    '双栈亮色令牌集合漂移 —— 两侧 :root 必须逐字对齐（R2）',
  );
});

test('暗色令牌集合：styles.css .dark 与 index.css .dark 完全相等', () => {
  const a = tokensOf(vanilla, '.dark');
  const b = tokensOf(react, '.dark');
  const onlyVanilla = [...a].filter((x) => !b.has(x));
  const onlyReact = [...b].filter((x) => !a.has(x));
  assert.deepEqual(
    { onlyVanilla, onlyReact },
    { onlyVanilla: [], onlyReact: [] },
    '双栈暗色令牌集合漂移 —— 两侧 .dark 必须逐字对齐（R2）',
  );
});

test('vellum 材质令牌在两栈都存在（E16 回归哨兵）', () => {
  assert(tokensOf(vanilla, ':root').has('--vellum-tint'));
  assert(tokensOf(react, ':root').has('--vellum-tint'));
});
