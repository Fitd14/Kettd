import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

/**
 * 标题下拉组件接线门禁（sticky-shell-polish · tasks 3.7）。
 *
 * `src-react/test/heading-select.test.mjs` 只跑 `heading-options.ts` 的纯函数（15 项），
 * 对组件本身零覆盖：谁把 `Select.Root` 换成原生 `<select>`、把 `onValueChange` 接错、
 * 或让 `tiptap-editor.tsx` 不再传 `applyHeading`，纯函数门禁照样全绿而界面已经坏掉。
 * 本文件静态锁住组件的 radix 接线与编辑器侧调用点，补上这段空白。
 *
 * 运行态行为（焦点、Tab、Esc、点选项）不在这里验证——那属于 L4 真机项（tasks 5.5）。
 */
const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const read = (p) => readFileSync(join(root, p), 'utf8');

const component = read('src-react/src/components/editor/heading-select.tsx');
const editor = read('src-react/src/rendering/tiptap-editor.tsx');
const css = read('src-react/src/rendering/styles/prosemirror.css');

test('组件用 radix Select（既有依赖）而非原生 select', () => {
  assert.match(component, /import \{ Select \} from 'radix-ui'/, '标题下拉不再从既有依赖 radix-ui 取 Select');
  assert.doesNotMatch(component, /<select[\s>]/, '组件回退成原生 <select>（OS 弹层，暗色主题无解）');
});

test('浮层走 popper 定位 + portal，不被编辑区裁剪', () => {
  assert.match(component, /Select\.Portal/, '浮层必须 portal 到 body，否则被编辑区滚动容器裁剪');
  assert.match(component, /position="popper"/, '浮层必须用 popper 定位（自动翻转 / 避让视口）');
  assert.match(component, /collisionPadding=\{\d+\}/, '缺少视口避让边距，浮层会贴边被裁');
});

test('选中即应用：onValueChange 必须转成 onSelect(option.value)', () => {
  assert.match(component, /onValueChange=\{\(next\) => \{[\s\S]{0,240}?onSelect\(option\.value\)/, '选中项没有回抛给编辑器');
  assert.match(component, /HEADING_OPTIONS\.map/, '选项必须来自纯逻辑模块的七项，不得组件内另立数组');
});

test('可访问性标记齐全', () => {
  assert.match(component, /aria-label=\{`\$\{ariaLabel\}：\$\{currentLabel\}`\}/, '触发按钮缺少「标题级别：当前值」可读名称');
  assert.match(component, /<Select\.Value>/, '触发按钮不再显示当前级别（回显丢失）');
});

test('编辑器侧接线：HeadingSelect 收 value 与 applyHeading', () => {
  assert.match(editor, /import \{ HeadingSelect \} from '@\/components\/editor\/heading-select'/, '编辑器不再引入标题下拉组件');
  assert.match(
    editor,
    /const applyHeading = \(next: HeadingValue\) => \{\s*applyHeadingCommand\(chain\(\), next\)\.run\(\)\s*\}/,
    'applyHeading 不再走纯逻辑的 applyHeadingCommand',
  );
  assert.match(editor, /<HeadingSelect value=\{headingValue\} onSelect=\{applyHeading\} \/>/, '编辑器给 HeadingSelect 的接线变了（value / onSelect 必须成对）');
});

test('样式只用 token，且不残留手写 listbox 的类名', () => {
  const menuRule = css.match(/\.kb-editor-heading-menu \{[\s\S]*?\n\}/);
  assert.ok(menuRule, '.kb-editor-heading-menu 样式块丢失');
  assert.doesNotMatch(menuRule[0], /#[0-9a-fA-F]{3,8}\b|rgba?\(/, '浮层样式出现裸色值（必须走 hsl(var(--token))）');
  assert.match(css, /\.kb-editor-heading-option\[data-highlighted\]|\[data-highlighted\]/, '缺少 radix data-highlighted 高亮映射');
  assert.doesNotMatch(css, /\.kb-editor-heading-option\.is-active/, '手写 listbox 时代的 .is-active 规则残留（radix 不产生该 class）');
});