import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { parseTheme, BUILTIN_THEME_IDS, THEME_VARS } from '../src-react/src/lib/reading-theme.js';

/**
 * 内置主题一致性（kb-theme-library 2.4 / 5.6）：后端 `READING_THEME_BUILTINS`、前端
 * `BUILTIN_THEME_IDS`、`src-react/src/assets/themes/*.css` 三处必须同集合；
 * 每套主题必须亮暗两块各 59 条白名单声明（外壳 19 + 阅读区 40）、0 忽略。
 */
const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const themeDir = join(root, 'src-react', 'src', 'assets', 'themes');
const modelsRs = readFileSync(join(root, 'src-tauri', 'src', 'models.rs'), 'utf8');

function rustBuiltins() {
  const start = modelsRs.indexOf('pub const READING_THEME_BUILTINS');
  assert.notEqual(start, -1, 'models.rs 必须定义 READING_THEME_BUILTINS');
  const block = modelsRs.slice(start, modelsRs.indexOf('];', start));
  return [...block.matchAll(/"([a-z0-9-]+)"/g)].map((match) => match[1]);
}

test('三处内置 id 完全一致', () => {
  const fromRust = rustBuiltins();
  assert.deepEqual(BUILTIN_THEME_IDS, fromRust, '前端 BUILTIN_THEME_IDS 必须与 models.rs 同序');
  for (const id of fromRust) {
    assert.ok(existsSync(join(themeDir, `${id}.css`)), `缺少内置主题文件 ${id}.css`);
  }
});

test('每个内置主题：亮暗两块各 59 条、0 忽略、元数据齐全', () => {
  for (const id of BUILTIN_THEME_IDS) {
    const css = readFileSync(join(themeDir, `${id}.css`), 'utf8');
    const theme = parseTheme(css);
    assert.equal(theme.ok, true, `${id}: ${theme.errors.join('；')}`);
    assert.ok(theme.name, `${id} 必须有 name`);
    assert.ok(theme.author, `${id} 必须有 author`);
    assert.ok(theme.description.includes('Apache'), `${id} 的来源与许可说明不能丢`);
    assert.equal(theme.ignored, 0, `${id} 有 ${theme.ignored} 条被忽略：${theme.ignoredNames.join(', ')}`);
    assert.equal(Object.keys(theme.light).length, THEME_VARS.length, `${id} 亮色声明数不对`);
    assert.equal(Object.keys(theme.dark).length, THEME_VARS.length, `${id} 暗色声明数不对`);
    // 全量口径：外壳必须真的换（否则「全应用」只是名义上的）
    for (const mode of ['light', 'dark']) {
      assert.match(theme[mode]['--background'], /^\d{1,3}(\.\d+)? \d{1,3}(\.\d+)?% \d{1,3}(\.\d+)?%$/, `${id} ${mode} 的 --background 必须是 HSL 三元组`);
      assert.match(theme[mode]['--foreground'], /^\d{1,3}(\.\d+)? \d{1,3}(\.\d+)?% \d{1,3}(\.\d+)?%$/, `${id} ${mode} 的 --foreground 必须是 HSL 三元组`);
    }
  }
});

test('两套主题的取值都在区间内（解析器已校验，这里抽查代表值）', () => {
  const paperglow = parseTheme(readFileSync(join(themeDir, 'paperglow.css'), 'utf8'));
  assert.equal(paperglow.light['--kb-paper-radius'], '16px');
  assert.equal(paperglow.light['--kb-line-height'], '1.7');
  assert.equal(paperglow.dark['--kb-paper-radius'], '16px');

  const mdmdt = parseTheme(readFileSync(join(themeDir, 'mdmdt.css'), 'utf8'));
  assert.equal(mdmdt.light['--kb-paper-radius'], '2px');
  assert.equal(mdmdt.light['--kb-table-zebra'], '#ECECEE');
  assert.equal(mdmdt.dark['--seal-track'], '#4F76DF');
});