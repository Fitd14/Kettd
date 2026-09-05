/**
 * 共享内核 · 提醒时刻编辑器（ADR-0006 第一增量）
 *
 * 为什么在 kernel 而不是留在 index.html：
 *  1. qa-3（v1 遗留多时刻不得静默降级为单时刻）的可证伪保证原先挂在
 *     `test/rem-editor.test.mjs` **从 index.html 抽源码文本**上 —— 那是 E15 风险，
 *     视图一搬进 React 就静默失效。抽成模块后测试改成 import，随迁移存活。
 *  2. 双轨期这套语义必须只有一份实现（vanilla 三窗 + React 共用）。
 *
 * 落库契约不变（V2-API §时间约定）：
 *   每天（循环） = HH:MM[/HH:MM...]   单次 = YYYY-MM-DDTHH:mm
 *
 * 纯函数：不读 S、不碰 document、不依赖 Tauri。渲染只返回 HTML 字符串。
 */
import { esc } from './text.js';

/** 错误文案的唯一来源。只讲下一步动作，不教手输格式（qa-2）。 */
export const REM_ERRORS = {
  noTime: '先选一个提醒时刻',
  needDate: '单次提醒要选日期；想每天提醒请切到「每天」',
  tooManyOnce: '单次提醒只能一个时刻，先删掉多余的',
};

/** 归一模式值：只认 'once'，其余一律按 'daily'。 */
export function normMode(mode) {
  return mode === 'once' ? 'once' : 'daily';
}

/**
 * 契约串 → 编辑器状态。**全量回填**，多时刻展开成等量槽位。
 * 这是 qa-3 的核心：进编辑不改值直接保存，必须原样写回。
 */
export function parseRemTime(raw) {
  const t = String(raw == null ? '' : raw).trim();
  if (t.includes('T')) {
    return { mode: 'once', date: t.slice(0, 10), times: [t.slice(11, 16)] };
  }
  const times = t.split('/').map((s) => s.trim().slice(0, 5)).filter(Boolean);
  return { mode: 'daily', date: '', times: times.length ? times : [''] };
}

/** 编辑器状态 → 契约串；返回 { value } 或 { err }。 */
export function composeRemTime({ mode, date, times }) {
  const m = normMode(mode);
  const picked = (Array.isArray(times) ? times : []).map((x) => String(x == null ? '' : x).trim()).filter(Boolean);
  if (!picked.length) return { err: REM_ERRORS.noTime };
  if (m === 'daily') return { value: picked.join('/') };
  const d = String(date == null ? '' : date).trim();
  if (!d) return { err: REM_ERRORS.needDate };
  if (picked.length > 1) return { err: REM_ERRORS.tooManyOnce };
  return { value: `${d}T${picked[0]}` };
}

/**
 * 切模式时的时刻收敛：进单次只留第一个已选时刻（不留隐藏多值），
 * 回每天时不吞掉已选时刻。返回新数组，不改入参。
 */
export function nextTimesForMode(times, mode) {
  const picked = (Array.isArray(times) ? times : []).map((t) => String(t == null ? '' : t).trim()).filter(Boolean);
  return normMode(mode) === 'once' ? [picked[0] || ''] : (picked.length ? picked : ['']);
}

/** 清空后的初始编辑器状态。 */
export function emptyRemDraft() {
  return { mode: 'daily', date: '', times: [''] };
}

/** 追加一个空槽位（返回新数组，不改入参）。 */
export function appendTimeSlot(times) {
  const next = (Array.isArray(times) ? times.slice() : []).concat(['']);
  return next.length ? next : [''];
}

/** 删除第 idx 个槽位；删空则自动补一个空槽（所以最后一个删不掉）。 */
export function removeTimeSlot(times, idx) {
  const next = (Array.isArray(times) ? times.slice() : ['']);
  next.splice(Number(idx) || 0, 1);
  if (!next.length) next.push('');
  return next;
}

/** 契约串的人话版：多时刻说个数，单次只说时刻不念日期前缀。 */
export function prettyRemTime(v) {
  const s = String(v || '');
  if (s.includes('/')) return `${s.split('/').length} 个时刻`;
  return s.includes('T') ? s.slice(11, 16) : s;
}
/**
 * 渲染编辑器 HTML。prefix 决定 data-in / data-act 命名（'rem' 添加行、'remEdit' 行内编辑）。
 * 只复用既有 .seg/.input/.btn/.row-flex 类，不新增样式与色值。
 */
export function renderRemEditor({ prefix, mode, date, times, invalid = false, compact = false }) {
  const m = normMode(mode);
  const slots = (Array.isArray(times) && times.length ? times : ['']);
  const bad = invalid ? ' invalid' : '';
  const sz = compact ? ' sm' : '';
  const w = (px) => `style="width:${px}px"`;
  const html = slots.map((v, i) => `<span class="row-flex" style="gap:2px">
      <input class="input mono${bad}${sz}" type="time" ${w(compact ? 96 : 100)} data-in="${prefix}Times" data-idx="${i}" value="${esc(v)}" aria-label="提醒时刻 ${i + 1}">
      ${m === 'daily' && slots.length > 1 ? `<button class="btn ghost xs" data-act="${prefix}-time-del" data-idx="${i}" aria-label="删掉第 ${i + 1} 个时刻">✕</button>` : ''}
    </span>`).join('');
  return `<span class="seg" role="group" aria-label="提醒方式">
      <button class="btn sm${m === 'daily' ? ' on' : ''}" data-act="${prefix}-mode" data-arg="daily" aria-pressed="${m === 'daily'}">每天</button>
      <button class="btn sm${m === 'once' ? ' on' : ''}" data-act="${prefix}-mode" data-arg="once" aria-pressed="${m === 'once'}">单次</button>
    </span>
    ${m === 'once' ? `<input class="input mono${bad}${sz}" type="date" ${w(compact ? 140 : 150)} data-in="${prefix}Date" value="${esc(date || '')}" aria-label="提醒日期">` : ''}
    ${html}
    ${m === 'daily' ? `<button class="btn ghost xs${sz}" data-act="${prefix}-time-add" aria-label="再加一个提醒时刻">+ 时刻</button>` : ''}`;
}
