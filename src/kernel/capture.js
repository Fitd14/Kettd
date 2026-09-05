/**
 * 共享内核 · 快录语法解析与组参（ADR-0006）
 *
 * 为什么在 kernel：`parseCapture` 原先藏在 api.js 里，而**快录组参逻辑被逐字复制了三份**
 * （index.html / float.html / capture.html 各一份）。三份复制已经造成过一次真实成本
 * （qa-2 文案改了两处漏一处）。这里收敛成一份，三窗与 src-react 共用。
 *
 * 契约（V2-API §8）：输出 title/chips/dueAt/autoRemind/category/priority/fellBack，
 * dueAt 全本地语义字符串，零 UTC 换算。
 */
import { pad2, localToday } from './time.js';

export function parseCapture(raw, today) {
  const t0 = today || localToday();
  let text = String(raw == null ? '' : raw).trim();
  const chips = [];
  let category;
  let priority;

  const cat = text.match(/#(工作|学习|生活)/);
  if (cat) {
    category = cat[1];
    chips.push({ kind: 'category', raw: cat[0], value: `分类 · ${category}`, confidence: 'high' });
    text = text.replace(cat[0], ' ');
  }

  const prio = text.match(/!(高|中|低)/);
  if (prio) {
    const pmap = { 高: 'high', 中: 'med', 低: 'low' };
    priority = pmap[prio[1]];
    chips.push({ kind: 'priority', raw: prio[0], value: `优先级 · ${prio[1]}`, confidence: 'high' });
    text = text.replace(prio[0], ' ');
  }

  // 时间：下午3点 / 15:00 / 晚上9点半 / 中午12点
  let hour = null;
  let minute = 0;
  const tm = text.match(/(早上|上午|中午|下午|晚上)\s*(\d{1,2})\s*[:：点]\s*(半|整|\d{1,2})?|(\d{1,2})[:：](\d{2})/);
  if (tm) {
    if (tm[4] !== undefined) {
      hour = parseInt(tm[4], 10);
      minute = parseInt(tm[5], 10);
    } else {
      hour = parseInt(tm[2], 10);
      const m = tm[3];
      minute = m === '半' ? 30 : (m && m !== '整' ? (parseInt(m, 10) || 0) : 0);
      const ap = tm[1];
      if ((ap === '下午' || ap === '晚上') && hour < 12) hour += 12;
      if (ap === '中午' && hour < 11) hour += 12;
    }
    if (hour > 23 || minute > 59) {
      hour = null;
      minute = 0;
    } else {
      chips.push({ kind: 'time', raw: tm[0].trim(), value: `时间 · ${pad2(hour)}:${pad2(minute)}`, confidence: 'high' });
      text = text.replace(tm[0], ' ');
    }
  }

  // 日期：今天/明天/后天 / 周X / 下周X / N月D日 / N.D
  const [ty, tmo, tda] = t0.split('-').map(Number);
  const base = new Date(ty, tmo - 1, tda);
  let dateISO = null;
  const dm = text.match(/(今天|明天|后天)/)
    || text.match(/(本周|下周|星期|周)([一二三四五六日天1-7])/)
    || text.match(/(\d{1,2})月(\d{1,2})[日号]/)
    || text.match(/(\d{1,2})[./](\d{1,2})(?=\s|$|[^(\d])/);
  if (dm) {
    const d = new Date(base);
    if (/^(今天|明天|后天)$/.test(dm[0])) {
      const off = { 今天: 0, 明天: 1, 后天: 2 };
      d.setDate(base.getDate() + off[dm[0]]);
    } else if (dm[2] !== undefined && /^(本周|下周|星期|周)/.test(dm[0])) {
      const wmap = { 一: 1, 二: 2, 三: 3, 四: 4, 五: 5, 六: 6, 日: 0, 天: 0, 1: 1, 2: 2, 3: 3, 4: 4, 5: 5, 6: 6, 7: 0 };
      const target = wmap[dm[2]] !== undefined ? wmap[dm[2]] : base.getDay();
      let diff = (target - base.getDay() + 7) % 7;
      if (dm[0].startsWith('下周')) diff += 7;
      d.setDate(base.getDate() + diff);
    } else {
      const nums = (dm[0].match(/\d+/g) || []).map(Number);
      if (nums.length === 2) d.setMonth(nums[0] - 1, nums[1]);
    }
    dateISO = `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
    chips.push({ kind: 'date', raw: dm[0], value: `日期 · ${dateISO}`, confidence: 'high' });
    text = text.replace(dm[0], ' ');
  }

  let dueAt = dateISO;
  if (dueAt && hour !== null) dueAt = `${dueAt}T${pad2(hour)}:${pad2(minute)}`;
  else if (!dueAt && hour !== null) dueAt = `${t0}T${pad2(hour)}:${pad2(minute)}`; // 只有时间：视为今天该时刻

  const title = text.replace(/\s+/g, ' ').trim() || String(raw == null ? '' : raw).trim();
  return {
    title,
    chips,
    dueAt,
    autoRemind: dueAt !== null, // 有截止即随截止提醒
    category,
    priority,
    fellBack: chips.length === 0,
  };
}

/**
 * 解析结果 → add_task 入参。
 * 这是原先散在 index.html / float.html / capture.html 的三份逐字复制的唯一实现。
 * 分类与优先级的兜底口径（生活 / med）也在这里，改口径只需改一处。
 */
export function buildCaptureArgs(parsed, source) {
  const args = {
    title: parsed.title,
    source: source || 'capture',
    category: parsed.category || '生活',
    priority: parsed.priority || 'med',
  };
  if (parsed.dueAt) {
    args.dueAt = parsed.dueAt;
    args.remindAt = parsed.autoRemind ? parsed.dueAt : null;
  }
  return args;
}
