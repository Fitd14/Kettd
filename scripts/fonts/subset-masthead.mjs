// 报头衬线子集生成器（开发时工具，不参与构建）。
//
// 输入：本机安装的 Noto Serif SC 可变字体（OFL 1.1），可用 FONT_SRC 覆盖路径。
// 输出：src-react/src/assets/fonts/kettd-masthead-serif.woff2
//
// 依赖 subset-font 只在生成时需要，不进 package.json：
//   npm i --no-save subset-font
//   node scripts/fonts/subset-masthead.mjs
//
// 字符表是「报头与品牌字的固定文案 + 数字 + 标点」。不要为了新文案随手加字：
// 这个家族只允许出现在 .view-title 与 .brand 两处（见 design.md §4.9）。
import { createRequire } from 'node:module';
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, '..', '..');
const require = createRequire(path.join(repoRoot, 'package.json'));
const subsetFont = require('subset-font');

const OUT_DIR = path.join(repoRoot, 'src-react', 'src', 'assets', 'fonts');
const SRC = process.env.FONT_SRC || 'C:/Windows/Fonts/NotoSerifSC-VF.ttf';

const MASTHEAD =
  '今天收件箱条未整理计划回顾知识库设置提醒安排详情周汇总草稿' +
  '0123456789' +
  '·，。、；：？！“”‘’（）【】《》—…％+';

async function main() {
  const src = await fs.readFile(SRC);
  const t0 = Date.now();
  const out = await subsetFont(src, MASTHEAD, {
    targetFormat: 'woff2',
    variationAxes: { wght: 600 },
  });
  await fs.mkdir(OUT_DIR, { recursive: true });
  const file = path.join(OUT_DIR, 'kettd-masthead-serif.woff2');
  await fs.writeFile(file, out);
  console.log(
    `masthead: ${(out.length / 1024).toFixed(1)} KB from ${(src.length / 1048576).toFixed(1)} MB in ${Date.now() - t0} ms`,
  );
  console.log(`chars: ${MASTHEAD.length}`);
}

main().catch((e) => {
  console.error('FAILED', e);
  process.exit(1);
});