// 便签移出淡化守卫单测（OpenSpec sticky-shell-polish tasks 4.1 / D5）。
// 直接 `node src-react/test/sticky-fade-guard.test.mjs` 运行（刻意不用 node:test / --test：
// Windows 沙箱下 node --test 的子进程管道会 EPERM）。Node 22 默认开启 type stripping，
// 可以直接 import 源码 .ts。
import assert from 'node:assert/strict'
import { FADE_DEBOUNCE_MS, isPointerOutside, shouldFade } from '../src/lib/sticky-fade.ts'

// ─── 极简测试运行器（与 sticky-text.test.mjs 同款）─────────────────────────

let passed = 0
const failures = []

function test(name, fn) {
  try {
    fn()
    passed += 1
    console.log(`✓ ${name}`)
  } catch (error) {
    failures.push({ name, error })
    console.error(`✗ ${name}\n    ${error.message}`)
  }
}

// ─── 四条件全组合 ────────────────────────────────────────────────────────

/** 4 个布尔条件共 16 种组合；期望值 = 指针在外 且 !hasFocus 且 !composing 且 !overlayOpen */
function conditionsOf(mask) {
  return {
    enabled: true,
    pointerOutside: (mask & 1) !== 0,
    hasFocus: (mask & 2) !== 0,
    composing: (mask & 4) !== 0,
    overlayOpen: (mask & 8) !== 0,
  }
}

test('四条件全组合（16 种）：仅「指针在外 + 失焦 + 未组字 + 无浮层」才淡化', () => {
  for (let mask = 0; mask < 16; mask += 1) {
    const conditions = conditionsOf(mask)
    const expected =
      conditions.pointerOutside && !conditions.hasFocus && !conditions.composing && !conditions.overlayOpen
    assert.equal(shouldFade(conditions), expected, `条件组合 mask=${mask} 判定不符`)
  }
})

test('四条件全组合：enabled=false 时 16 种组合恒不淡化', () => {
  for (let mask = 0; mask < 16; mask += 1) {
    assert.equal(shouldFade({ ...conditionsOf(mask), enabled: false }), false, `mask=${mask} 不应淡化`)
  }
})

// ─── 单条件否决 ──────────────────────────────────────────────────────────

test('四条件全满足（指针在外 + 失焦 + 未组字 + 无浮层）→ 淡化', () => {
  assert.equal(
    shouldFade({ enabled: true, pointerOutside: true, hasFocus: false, composing: false, overlayOpen: false }),
    true,
  )
})

test('指针在窗口内 → 不淡化', () => {
  assert.equal(
    shouldFade({ enabled: true, pointerOutside: false, hasFocus: false, composing: false, overlayOpen: false }),
    false,
  )
})

test('窗口仍有焦点 → 不淡化（含鼠标移出但窗口未失焦）', () => {
  assert.equal(
    shouldFade({ enabled: true, pointerOutside: true, hasFocus: true, composing: false, overlayOpen: false }),
    false,
  )
})

test('输入法组字中 → 不淡化（候选窗误触 mouseleave 的主防护）', () => {
  assert.equal(
    shouldFade({ enabled: true, pointerOutside: true, hasFocus: false, composing: true, overlayOpen: false }),
    false,
  )
})

test('窗口内有浮层（链接弹窗 / slash 命令面板 / 选区气泡）→ 不淡化', () => {
  assert.equal(
    shouldFade({ enabled: true, pointerOutside: true, hasFocus: false, composing: false, overlayOpen: true }),
    false,
  )
})

test('设置关闭「移出淡化」→ 即便其余条件全满足也不淡化', () => {
  assert.equal(
    shouldFade({ enabled: false, pointerOutside: true, hasFocus: false, composing: false, overlayOpen: false }),
    false,
  )
})

// ─── 去抖常量 ────────────────────────────────────────────────────────────

test('去抖常量为 250ms（design D5 的 200–300ms 区间内）', () => {
  assert.equal(FADE_DEBOUNCE_MS, 250)
  assert.ok(FADE_DEBOUNCE_MS >= 200 && FADE_DEBOUNCE_MS <= 300)
})

// ─── 指针边界 ────────────────────────────────────────────────────────────

const W = 380
const H = 300

test('窗口内坐标 → 不算移出（含四角内侧 1px）', () => {
  assert.equal(isPointerOutside(1, 1, W, H), false)
  assert.equal(isPointerOutside(W - 1, H - 1, W, H), false)
  assert.equal(isPointerOutside(190, 150, W, H), false)
  assert.equal(isPointerOutside(0.5, 10, W, H), false)
})

test('贴着窗口边界（<= 0 或 >= 宽/高）→ 算移出', () => {
  assert.equal(isPointerOutside(0, 10, W, H), true)
  assert.equal(isPointerOutside(10, 0, W, H), true)
  assert.equal(isPointerOutside(W, 10, W, H), true)
  assert.equal(isPointerOutside(10, H, W, H), true)
})

test('越过边界的负坐标 / 超出宽高 → 算移出（四角全覆盖）', () => {
  assert.equal(isPointerOutside(-1, -1, W, H), true)
  assert.equal(isPointerOutside(W + 20, -1, W, H), true)
  assert.equal(isPointerOutside(-1, H + 20, W, H), true)
  assert.equal(isPointerOutside(W + 20, H + 20, W, H), true)
})

test('非有限数 / 非法窗口尺寸 → 按窗内处理（避免误淡化）', () => {
  assert.equal(isPointerOutside(Number.NaN, 10, W, H), false)
  assert.equal(isPointerOutside(10, Number.POSITIVE_INFINITY, W, H), false)
  assert.equal(isPointerOutside(10, 10, Number.NaN, H), false)
  assert.equal(isPointerOutside(10, 10, W, 0), false)
})

// ─── 汇总 ────────────────────────────────────────────────────────────────

console.log('')
if (failures.length > 0) {
  console.error(`失败 ${failures.length} 项 / 通过 ${passed} 项`)
  process.exit(1)
}
console.log(`全部通过：${passed} 项`)