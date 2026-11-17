/**
 * 外观主题 · 解析与应用（kb-theme-library 2.1 / 2.2 / 5.1）
 *
 * 主题文件是「CSS 子集」：只认 `.kb-theme-light` / `.kb-theme-dark` 两块里的
 * 59 个白名单自定义属性（design.md D3：外壳 19 + 阅读 40），其余选择器与 at-rule
 * 一律不生效；值按通用 / 数值 / 圆角 / 结构 / 颜色 / HSL 三元组六套规则校验，
 * 越界丢弃该条并计入 ignored（design.md D3「越界丢弃，不整份拒收」）。
 *
 * 为什么不注入整份 CSS：作用域不可控（能改到 chrome）、注入面太大（任意选择器 /
 * `@import` / `url()`）。这里只把通过校验的声明逐条 `setProperty` 到
 * `document.documentElement`（全量口径，design.md D2 / D10），外壳与阅读区都靠
 * CSS 继承吃到主题。
 *
 * 文件用 .js 而非 .ts：解析器要能被 `node --test` 直接 import 做单测（同
 * `kernel/selectors.js` 的先例），构建侧由 Vite 原样接入。
 */

/** 主题文件头声明的字段（首段注释里的 `kettd-theme` 块）。 */
const META_KEYS = ['name', 'author', 'description'];

/** 内置主题 id：与 `src-tauri/src/models.rs` 的 `READING_THEME_BUILTINS` 一致。 */
export const BUILTIN_THEME_IDS = ['paperglow', 'mdmdt'];

/** 内置主题 id → 展示名（设置页用；未收录的 id 直接显示 id）。 */
export const BUILTIN_THEME_LABELS = {
  paperglow: 'Paperglow · 暖纸陶土',
  mdmdt: 'Mdmdt · 冷白皇家蓝',
};

const RADIUS_MIN = { '--kb-paper-radius': 0, '--kb-quote-radius': 0, '--kb-code-radius': 0, '--kb-inline-code-radius': 0, '--kb-table-radius': 0, '--kb-img-radius': 0 };
const RADIUS_MAX = { '--kb-paper-radius': 24, '--kb-quote-radius': 16, '--kb-code-radius': 16, '--kb-inline-code-radius': 8, '--kb-table-radius': 16, '--kb-img-radius': 16 };

/**
 * 59 个白名单变量，按组排列（顺序即设置页/文档里的展示顺序）。
 * kind = hsl | color | number | radius | rule | shadow | font
 */
export const THEME_VARS = [
  // 外壳配色（19）：取值是不带 hsl() 的三元组，与 index.css 的 :root 同形
  { name: '--background', kind: 'hsl', group: '外壳配色' },
  { name: '--foreground', kind: 'hsl', group: '外壳配色' },
  { name: '--card', kind: 'hsl', group: '外壳配色' },
  { name: '--card-foreground', kind: 'hsl', group: '外壳配色' },
  { name: '--popover', kind: 'hsl', group: '外壳配色' },
  { name: '--popover-foreground', kind: 'hsl', group: '外壳配色' },
  { name: '--primary', kind: 'hsl', group: '外壳配色' },
  { name: '--primary-foreground', kind: 'hsl', group: '外壳配色' },
  { name: '--secondary', kind: 'hsl', group: '外壳配色' },
  { name: '--secondary-foreground', kind: 'hsl', group: '外壳配色' },
  { name: '--muted', kind: 'hsl', group: '外壳配色' },
  { name: '--muted-foreground', kind: 'hsl', group: '外壳配色' },
  { name: '--accent', kind: 'hsl', group: '外壳配色' },
  { name: '--accent-foreground', kind: 'hsl', group: '外壳配色' },
  { name: '--border', kind: 'hsl', group: '外壳配色' },
  { name: '--input', kind: 'hsl', group: '外壳配色' },
  { name: '--ring', kind: 'hsl', group: '外壳配色' },
  { name: '--link', kind: 'hsl', group: '外壳配色' },
  { name: '--seal', kind: 'hsl', group: '外壳配色' },
  // 纸面与墨色（8）
  { name: '--paper', kind: 'color', group: '纸面与墨色' },
  { name: '--ink-1', kind: 'color', group: '纸面与墨色' },
  { name: '--ink-2', kind: 'color', group: '纸面与墨色' },
  { name: '--ink-3', kind: 'color', group: '纸面与墨色' },
  { name: '--line-warm', kind: 'color', group: '纸面与墨色' },
  { name: '--seal-track', kind: 'color', group: '纸面与墨色' },
  { name: '--seal-ink', kind: 'color', group: '纸面与墨色' },
  { name: '--seal-soft', kind: 'color', group: '纸面与墨色' },
  // 纸面形态（3）
  { name: '--kb-paper-radius', kind: 'radius', group: '纸面形态' },
  { name: '--kb-paper-shadow', kind: 'shadow', group: '纸面形态' },
  { name: '--kb-paper-texture', kind: 'number', group: '纸面形态', min: 0, max: 0.12 },
  // 字族（2）
  { name: '--font-reading', kind: 'font', group: '字族' },
  { name: '--font-reading-title', kind: 'font', group: '字族' },
  // 段落（1）
  { name: '--kb-line-height', kind: 'number', group: '段落', min: 1.3, max: 2.4 },
  // 标题（9）
  { name: '--kb-h1', kind: 'number', group: '标题', min: 0.8, max: 2 },
  { name: '--kb-h2', kind: 'number', group: '标题', min: 0.8, max: 2 },
  { name: '--kb-h3', kind: 'number', group: '标题', min: 0.8, max: 2 },
  { name: '--kb-h4', kind: 'number', group: '标题', min: 0.8, max: 2 },
  { name: '--kb-h5', kind: 'number', group: '标题', min: 0.8, max: 2 },
  { name: '--kb-h6', kind: 'number', group: '标题', min: 0.8, max: 2 },
  { name: '--kb-head-tracking', kind: 'number', group: '标题', min: -0.02, max: 0.2, unit: 'em' },
  { name: '--kb-h1-rule', kind: 'rule', group: '标题', maxWidth: 4 },
  { name: '--kb-h2-rule', kind: 'rule', group: '标题', maxWidth: 4 },
  // 引用（4）
  { name: '--kb-quote-bg', kind: 'color', group: '引用' },
  { name: '--kb-quote-line', kind: 'rule', group: '引用', maxWidth: 6, required: true },
  { name: '--kb-quote-ink', kind: 'color', group: '引用' },
  { name: '--kb-quote-radius', kind: 'radius', group: '引用' },
  // 代码（6）
  { name: '--kb-code-bg', kind: 'color', group: '代码' },
  { name: '--kb-code-ink', kind: 'color', group: '代码' },
  { name: '--kb-code-radius', kind: 'radius', group: '代码' },
  { name: '--kb-inline-code-bg', kind: 'color', group: '代码' },
  { name: '--kb-inline-code-ink', kind: 'color', group: '代码' },
  { name: '--kb-inline-code-radius', kind: 'radius', group: '代码' },
  // 表格（4）
  { name: '--kb-table-line', kind: 'color', group: '表格' },
  { name: '--kb-table-head-bg', kind: 'color', group: '表格' },
  { name: '--kb-table-zebra', kind: 'color', group: '表格' },
  { name: '--kb-table-radius', kind: 'radius', group: '表格' },
  // 链接（2）
  { name: '--kb-link-ink', kind: 'color', group: '链接' },
  { name: '--kb-link-underline', kind: 'number', group: '链接', min: 0, max: 2, unit: 'px', required: true },
  // 图片（1）
  { name: '--kb-img-radius', kind: 'radius', group: '图片' },
];

const VAR_BY_NAME = new Map(THEME_VARS.map((meta) => [meta.name, meta]));

/** 每个主题文件都必须同时声明这两块，缺一块用另一块兜底（design.md D2 三级回退）。 */
export const THEME_CLASSES = { light: 'kb-theme-light', dark: 'kb-theme-dark' };

export const MAX_VALUE_LENGTH = 200;

/** 通用禁用串（小写比较）：外部资源、表达式、结构逃逸、档位令牌夹带。 */
const FORBIDDEN = ['url(', '@', '{', '}', ';', 'expression(', 'javascript:', '<', '>', '!important', 'var(--fs-', 'var(--ls-', 'var(--kb-measure', 'var(--kb-gutter'];

/** 文件级禁用串：出现即整份拒收（后端 save_reading_theme 同一份清单）。 */
const FILE_FORBIDDEN = ['@import', 'url(', 'expression(', 'javascript:'];

const COLOR_PATTERNS = [
  /^#[0-9a-fA-F]{3,8}$/,
  /^rgba?\([^()]*\)$/,
  /^hsla?\([^()]*\)$/,
  /^var\(--[a-zA-Z0-9-]+(?:\s*,\s*[^()]*)?\)$/,
  /^[a-zA-Z]+$/,
];

const BORDER_STYLES = new Set(['solid', 'dashed', 'dotted']);

function isForbidden(value) {
  const lowered = value.toLowerCase();
  return FORBIDDEN.some((needle) => lowered.includes(needle));
}

function parseNumber(value, unit) {
  const trimmed = value.trim();
  if (unit) {
    if (!trimmed.toLowerCase().endsWith(unit)) return null;
    const body = trimmed.slice(0, -unit.length).trim();
    return /^[+-]?(\d+(\.\d+)?|\.\d+)$/.test(body) ? Number(body) : null;
  }
  return /^[+-]?(\d+(\.\d+)?|\.\d+)$/.test(trimmed) ? Number(trimmed) : null;
}

function parseRadius(value, meta) {
  const parts = value.trim().split(/\s+/);
  if (parts.length < 1 || parts.length > 4) return false;
  const min = RADIUS_MIN[meta.name];
  const max = RADIUS_MAX[meta.name];
  return parts.every((part) => {
    if (part === '0') return min === 0;
    const px = parseNumber(part, 'px');
    return px !== null && px >= min && px <= max;
  });
}

function parseRule(value, meta) {
  const trimmed = value.trim().toLowerCase();
  if (trimmed === 'none') return !meta.required;
  const match = trimmed.match(/^(\d+(?:\.\d+)?)px\s+(solid|dashed|dotted)\s+(.+)$/);
  if (!match) return false;
  const width = Number(match[1]);
  return width <= meta.maxWidth && match[2] !== '' && BORDER_STYLES.has(match[2]);
}

/** box-shadow：`none` 或若干层「偏移 + 可选 inset + 颜色」。只做形态与长度校验。 */
function parseShadow(value) {
  const trimmed = value.trim();
  if (trimmed.toLowerCase() === 'none') return true;
  if (trimmed.length > MAX_VALUE_LENGTH) return false;
  const layers = splitTopLevel(trimmed, ',').map((layer) => layer.trim()).filter(Boolean);
  if (layers.length === 0 || layers.length > 4) return false;
  return layers.every((layer) => {
    if (!/\d/.test(layer) || !/\b(?:inset|rgba?|hsla?|#|[a-zA-Z]+)\b/.test(layer)) return false;
    // 只允许数字、px、颜色函数与少量连接符，挡住 calc()/var() 以外的花活
    return /^[0-9a-zA-Z#(),.%\s-]+$/.test(layer);
  });
}

/** 按顶层分隔符切分（括号内的逗号不算），用于 box-shadow 层与字族栈。 */
function splitTopLevel(text, separator) {
  const parts = [];
  let depth = 0;
  let start = 0;
  for (let index = 0; index < text.length; index += 1) {
    const char = text[index];
    if (char === '(') depth += 1;
    else if (char === ')') depth = Math.max(0, depth - 1);
    else if (char === separator && depth === 0) {
      parts.push(text.slice(start, index));
      start = index + 1;
    }
  }
  parts.push(text.slice(start));
  return parts;
}

function parseFont(value) {
  const trimmed = value.trim();
  if (!trimmed) return false;
  // 字族栈：逗号分隔，每项是带引号的族名或裸族名（不含函数）
  return splitTopLevel(trimmed, ',').every((part) => {
    const face = part.trim();
    if (!face) return false;
    if (/^"[^"]+"$/.test(face) || /^'[^']+'$/.test(face)) return true;
    return /^[a-zA-Z][a-zA-Z0-9 \-]*$/.test(face);
  });
}

function parseColor(value) {
  const trimmed = value.trim();
  return COLOR_PATTERNS.some((pattern) => pattern.test(trimmed));
}

/** HSL 三元组：`H S% L%`（不带 hsl()），H 0–360、S/L 0–100。 */
const HSL_TRIPLET = /^(\d{1,3}(?:\.\d+)?)\s+(\d{1,3}(?:\.\d+)?)%\s+(\d{1,3}(?:\.\d+)?)%$/;

function parseHslTriplet(value) {
  const match = value.trim().match(HSL_TRIPLET);
  if (!match) return false;
  const hue = Number(match[1]);
  const saturation = Number(match[2]);
  const lightness = Number(match[3]);
  return hue <= 360 && saturation <= 100 && lightness <= 100;
}

/** 单条声明的校验；通过返回规范化后的值，否则 null。 */
export function validateDeclaration(name, rawValue) {
  const meta = VAR_BY_NAME.get(name);
  if (!meta) return null;
  const value = rawValue.trim();
  if (!value || value.length > MAX_VALUE_LENGTH) return null;
  if (isForbidden(value)) return null;
  switch (meta.kind) {
    case 'hsl':
      return parseHslTriplet(value) ? value : null;
    case 'color':
      return parseColor(value) ? value : null;
    case 'number': {
      const num = parseNumber(value, meta.unit);
      return num !== null && num >= meta.min && num <= meta.max ? value : null;
    }
    case 'radius':
      return parseRadius(value, meta) ? value : null;
    case 'rule':
      return parseRule(value, meta) ? value : null;
    case 'shadow':
      return parseShadow(value) ? value : null;
    case 'font':
      return parseFont(value) ? value : null;
    default:
      return null;
  }
}

/** 去掉 CSS 注释，同时保留注释文本用于读文件头。 */
function splitComments(css) {
  const comments = [];
  const body = css.replace(/\/\*([\s\S]*?)\*\//g, (_, text) => {
    comments.push(text);
    return '';
  });
  return { body, comments };
}

/** 扫描顶层 `{…}` 块（值里出现 `{}` 会被通用规则拒收，这里按深度切即可）。 */
function readBlocks(body) {
  const blocks = [];
  let index = 0;
  while (index < body.length) {
    const open = body.indexOf('{', index);
    if (open === -1) break;
    const selector = body.slice(index, open).trim();
    let depth = 1;
    let cursor = open + 1;
    while (cursor < body.length && depth > 0) {
      if (body[cursor] === '{') depth += 1;
      else if (body[cursor] === '}') depth -= 1;
      cursor += 1;
    }
    const inner = body.slice(open + 1, cursor - 1);
    blocks.push({ selector, inner });
    index = cursor;
  }
  return blocks;
}

function readMeta(comments) {
  for (const text of comments) {
    if (!text.includes('kettd-theme')) continue;
    const meta = {};
    for (const line of text.split(/\r?\n/)) {
      const match = line.match(/^\s*([a-zA-Z-]+)\s*:\s*(.+)$/);
      if (match && META_KEYS.includes(match[1].toLowerCase())) {
        meta[match[1].toLowerCase()] = match[2].trim();
      }
    }
    // 单行写法：`kettd-theme name: X author: Y`
    if (!meta.name) {
      const inline = text.match(/kettd-theme\s+name\s*:\s*([^\n]+)/);
      if (inline) meta.name = inline[1].trim();
    }
    return meta;
  }
  return {};
}

function readDeclarations(inner, ignored) {
  const values = {};
  for (const chunk of inner.split(';')) {
    const text = chunk.trim();
    if (!text) continue;
    const colon = text.indexOf(':');
    if (colon === -1) {
      ignored.count += 1;
      continue;
    }
    const name = text.slice(0, colon).trim();
    const value = text.slice(colon + 1).trim();
    if (!name.startsWith('--')) continue;
    const checked = validateDeclaration(name, value);
    if (checked === null) {
      ignored.count += 1;
      ignored.names.push(name);
      continue;
    }
    values[name] = checked;
  }
  return values;
}

/**
 * 解析一份主题 CSS。
 *
 * @param {string} css 主题文件全文
 * @returns {{ ok: boolean, id?: string, name: string, author: string, description: string,
 *   light: Record<string,string>, dark: Record<string,string>,
 *   ignored: number, ignoredNames: string[], errors: string[] }}
 */
export function parseTheme(css) {
  const errors = [];
  const ignoredNames = [];
  const ignored = { count: 0, names: ignoredNames };
  const text = String(css ?? '').replace(/^\uFEFF/, '');
  const { body, comments } = splitComments(text);
  const lowered = text.toLowerCase();
  if (FILE_FORBIDDEN.some((needle) => lowered.includes(needle))) {
    errors.push('主题里不允许引用外部资源或表达式');
  }
  const meta = readMeta(comments);
  if (!meta.name) errors.push('主题文件缺少 name 声明（文件头 kettd-theme 块）');

  const blocks = readBlocks(body);
  const themes = { light: {}, dark: {} };
  for (const block of blocks) {
    const selector = block.selector.replace(/\s+/g, '');
    if (selector === `.${THEME_CLASSES.light}`) {
      Object.assign(themes.light, readDeclarations(block.inner, ignored));
    } else if (selector === `.${THEME_CLASSES.dark}`) {
      Object.assign(themes.dark, readDeclarations(block.inner, ignored));
    }
    // 其它选择器整块忽略（不是错误：主题文件允许携带自己的注释性样式）
  }
  const declared = Object.keys(themes.light).length + Object.keys(themes.dark).length;
  if (declared === 0) errors.push('主题里没有可用的亮色或暗色声明');

  return {
    ok: errors.length === 0,
    name: meta.name ?? '',
    author: meta.author ?? '',
    description: meta.description ?? '',
    light: themes.light,
    dark: themes.dark,
    ignored: ignored.count,
    ignoredNames,
    errors,
  };
}

/** 已应用到某个元素上的属性名，卸载时只删自己写过的（不误伤别处的内联样式）。 */
const appliedProps = new WeakMap();

/**
 * 把主题写到目标根节点（全量口径下固定传 `document.documentElement`）。
 *
 * 选套规则（design.md D2 / D8）：当前明暗有就用当前，没有就用另一套（亮暗共用一套
 * 配色的主题很常见），两套都没有就清空回到默认外观。`userFont` 非空时跳过两个字族
 * 变量——用户显式选的字族优先于主题。
 *
 * @param {HTMLElement | null} el 应用根节点（`document.documentElement`）
 * @param {ReturnType<typeof parseTheme> | null} theme 解析结果
 * @param {{ dark?: boolean, userFont?: string }} [options]
 */
export function applyTheme(el, theme, options = {}) {
  if (!el) return null;
  const dark = !!options.dark;
  const userFont = String(options.userFont ?? '').trim();
  const source = theme ? (dark ? theme.dark : theme.light) : null;
  const fallback = theme ? (dark ? theme.light : theme.dark) : null;
  const chosen = source && Object.keys(source).length > 0 ? source : fallback;
  clearTheme(el, { keepDataset: true });
  if (!chosen) {
    el.removeAttribute('data-reading-theme');
    return null;
  }
  const applied = [];
  for (const [name, value] of Object.entries(chosen)) {
    if (userFont && (name === '--font-reading' || name === '--font-reading-title')) continue;
    el.style.setProperty(name, value);
    applied.push(name);
  }
  appliedProps.set(el, applied);
  el.setAttribute('data-reading-theme', theme?.name ?? '');
  return applied;
}

/** 卸载主题：只清掉 applyTheme 写进去的那些属性。 */
export function clearTheme(el, options = {}) {
  if (!el) return;
  const applied = appliedProps.get(el) ?? [];
  for (const name of applied) el.style.removeProperty(name);
  appliedProps.delete(el);
  if (!options.keepDataset) {
    el.removeAttribute('data-reading-theme');
  }
}

/**
 * 主题里的某条声明是否真的会被读到（设置页「识别 N 条」用）。
 * 与 `applyTheme` 无关，只是方便 UI 统计。
 */
export function countDeclarations(theme, dark = false) {
  if (!theme) return 0;
  const values = dark ? theme.dark : theme.light;
  return values ? Object.keys(values).length : 0;
}