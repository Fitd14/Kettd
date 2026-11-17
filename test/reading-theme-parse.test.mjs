import test from 'node:test';
import assert from 'node:assert/strict';
import {
  parseTheme,
  validateDeclaration,
  applyTheme,
  clearTheme,
  THEME_VARS,
  BUILTIN_THEME_IDS,
} from '../src-react/src/lib/reading-theme.js';

/** 最小合法主题：文件头 + 亮暗两块各一条。 */
const SAMPLE = `/* kettd-theme
   name: 测试主题
   author: 测试
   description: 只用于单测
*/
.kb-theme-light {
  --paper: #FCFAF6;
  --kb-line-height: 1.7;
}
.kb-theme-dark {
  --paper: #2B2926;
  --kb-line-height: 1.8;
}
`;

test('白名单恰好 59 个变量且不重复', () => {
  assert.equal(THEME_VARS.length, 59);
  const names = THEME_VARS.map((meta) => meta.name);
  assert.equal(new Set(names).size, 59);
  assert.ok(!names.includes('--paper-deep'), '--paper-deep 有声明无消费，不得进白名单');
  assert.ok(!names.includes('--fs-reading'), '字号档位令牌不得进白名单');
  assert.ok(!names.includes('--kb-measure-cap'), '列宽令牌不得进白名单');
  assert.ok(!names.includes('--destructive'), '语义色不归主题管（全量口径 D3）');
  assert.ok(!names.includes('--bubble-bg'), '气泡底固定深底，不归主题管');
});

test('外壳配色必须是 HSL 三元组（不带 hsl()）', () => {
  assert.equal(validateDeclaration('--background', '34 37% 93%'), '34 37% 93%');
  assert.equal(validateDeclaration('--foreground', '240 6% 10%'), '240 6% 10%');
  assert.equal(validateDeclaration('--border', '360 100% 100%'), '360 100% 100%');
  // 带函数、超界、写成 hex 都不受理
  assert.equal(validateDeclaration('--background', 'hsl(34 37% 93%)'), null);
  assert.equal(validateDeclaration('--background', '400 37% 93%'), null);
  assert.equal(validateDeclaration('--background', '34 120% 93%'), null);
  assert.equal(validateDeclaration('--background', '#FCFAF6'), null);
});

test('内置 id 与后端白名单同序', () => {
  assert.deepEqual(BUILTIN_THEME_IDS, ['paperglow', 'mdmdt']);
});

test('合法主题：读文件头、亮暗两块、0 忽略', () => {
  const theme = parseTheme(SAMPLE);
  assert.equal(theme.ok, true, theme.errors.join('；'));
  assert.equal(theme.name, '测试主题');
  assert.equal(theme.author, '测试');
  assert.equal(Object.keys(theme.light).length, 2);
  assert.equal(Object.keys(theme.dark).length, 2);
  assert.equal(theme.ignored, 0);
  assert.equal(theme.light['--paper'], '#FCFAF6');
});

test('缺 name 判为错误', () => {
  const theme = parseTheme('.kb-theme-light { --paper: #fff; }');
  assert.equal(theme.ok, false);
  assert.ok(theme.errors.some((item) => item.includes('name')));
});

test('白名单外变量被忽略并计数', () => {
  const theme = parseTheme(`/* kettd-theme name: X */
.kb-theme-light {
  --paper: #fff;
  --paper-deep: #000;
  --fs-reading: 20px;
  --kb-measure-cap: 200ch;
}
`);
  assert.equal(theme.ok, true);
  assert.equal(Object.keys(theme.light).length, 1);
  assert.equal(theme.ignored, 3);
  assert.deepEqual(theme.ignoredNames.sort(), ['--fs-reading', '--kb-measure-cap', '--paper-deep']);
});

test('外部资源与表达式整份拒收', () => {
  const imported = parseTheme(`/* kettd-theme name: X */
@import url(https://evil.example/a.css);
.kb-theme-light { --paper: #fff; }
`);
  assert.equal(imported.ok, false);
  assert.ok(imported.errors.some((item) => item.includes('外部资源')));

  const urlValue = parseTheme(`/* kettd-theme name: X */
.kb-theme-light { --paper: url(https://evil.example/a.png); }
`);
  assert.equal(urlValue.ok, false);
});

test('越界值逐条丢弃：400px 圆角、5px double 引用线', () => {
  assert.equal(validateDeclaration('--kb-quote-radius', '400px'), null);
  assert.equal(validateDeclaration('--kb-quote-line', '5px double red'), null);
  assert.equal(validateDeclaration('--kb-quote-line', '4px solid #3E69D7'), '4px solid #3E69D7');
  assert.equal(validateDeclaration('--kb-h1', '2.4'), null);
  assert.equal(validateDeclaration('--kb-h1', '1.9'), '1.9');
  assert.equal(validateDeclaration('--kb-line-height', '1.2'), null);
});

test('值里夹带档位令牌或结构逃逸被拒', () => {
  assert.equal(validateDeclaration('--paper', 'var(--fs-reading)'), null);
  assert.equal(validateDeclaration('--kb-paper-shadow', '0 0 1px red !important'), null);
  assert.equal(validateDeclaration('--kb-quote-line', '3px solid red; }'), null);
  assert.equal(validateDeclaration('--kb-table-radius', '0 8px 8px 0'), '0 8px 8px 0');
  assert.equal(validateDeclaration('--kb-paper-shadow', 'none'), 'none');
});

/** 极简元素桩：只实现 applyTheme 用到的接口。 */
function fakeElement() {
  const props = new Map();
  return {
    style: {
      setProperty(name, value) {
        props.set(name, value);
      },
      removeProperty(name) {
        props.delete(name);
      },
      getPropertyValue(name) {
        return props.get(name) ?? '';
      },
    },
    attrs: new Map(),
    setAttribute(name, value) {
      this.attrs.set(name, value);
    },
    removeAttribute(name) {
      this.attrs.delete(name);
    },
    props,
  };
}

test('applyTheme 按明暗选套、缺套回落，userFont 跳过字族', () => {
  const theme = parseTheme(`/* kettd-theme name: X */
.kb-theme-light { --paper: #fff; --font-reading: "Song", serif; }
.kb-theme-dark { --paper: #111; }
`);
  const lightEl = fakeElement();
  applyTheme(lightEl, theme, { dark: false, userFont: 'system:KaiTi' });
  assert.equal(lightEl.props.get('--paper'), '#fff');
  assert.equal(lightEl.props.has('--font-reading'), false, '用户选了字族时主题字族不得生效');
  assert.equal(lightEl.attrs.get('data-reading-theme'), 'X');

  const darkEl = fakeElement();
  applyTheme(darkEl, theme, { dark: true });
  assert.equal(darkEl.props.get('--paper'), '#111');

  // 暗色套缺 paper 时用亮色套兜底
  const sparse = parseTheme(`/* kettd-theme name: Y */
.kb-theme-light { --paper: #fff; --ink-1: #000; }
.kb-theme-dark { --paper: #111; }
`);
  const el = fakeElement();
  applyTheme(el, sparse, { dark: true });
  assert.equal(el.props.get('--paper'), '#111');
  assert.equal(el.props.has('--ink-1'), false, '缺省不得从另一套补齐单个变量');
});

test('clearTheme 只清自己写过的属性', () => {
  const theme = parseTheme(`/* kettd-theme name: X */
.kb-theme-light { --paper: #fff; }
`);
  const el = fakeElement();
  el.style.setProperty('--ink-1', '#手工设置的');
  applyTheme(el, theme, { dark: false });
  clearTheme(el);
  assert.equal(el.props.has('--paper'), false);
  assert.equal(el.props.get('--ink-1'), '#手工设置的');
  assert.equal(el.attrs.has('data-reading-theme'), false);
});