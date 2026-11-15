import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

/** 打包配置契约：版本单一来源、Tauri 冲突点、脚本入口、依赖清单不许回退。
 *  这些值都是「改错了要到发版当天才发现」的，钉成门禁比写进文档可靠。 */
const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const read = (p) => readFileSync(join(root, p), 'utf8');
const readJson = (p) => JSON.parse(read(p));
const exists = (p) => existsSync(join(root, p));

const rootPkg = readJson('package.json');
const frontPkg = readJson('src-react/package.json');
const conf = readJson('src-tauri/tauri.conf.json');
const cargoToml = read('src-tauri/Cargo.toml');

/** 取 Cargo.toml [package] 段的某个键 */
const cargoPackageField = (key) => {
  const pkg = cargoToml.split(/^\[/m).find((s) => s.startsWith('package]'));
  assert.ok(pkg, 'Cargo.toml 缺 [package] 段');
  const m = pkg.match(new RegExp(`^${key}\\s*=\\s*"([^"]+)"`, 'm'));
  return m ? m[1] : undefined;
};

test('产品版本四处一致（package.json / src-react / tauri.conf / Cargo.toml）', () => {
  const versions = {
    'package.json': rootPkg.version,
    'src-react/package.json': frontPkg.version,
    'tauri.conf.json': conf.package.version,
    'Cargo.toml': cargoPackageField('version'),
  };
  const unique = new Set(Object.values(versions));
  assert.equal(
    unique.size,
    1,
    `版本漂移：${Object.entries(versions).map(([k, v]) => `${k}=${v}`).join(' / ')}；用 node scripts/version.mjs set <x.y.z> 统一`,
  );
});

test('Tauri 打包配置保持可用（钩子 / 产物目录 / WiX 语言）', () => {
  assert.equal(conf.build.beforeBuildCommand, 'npm run build:front');
  assert.equal(conf.build.devPath, '../src-react/dist');
  assert.equal(conf.build.distDir, '../src-react/dist');
  assert.equal(conf.package.productName, 'Kettd');
  assert.equal(conf.tauri.bundle.active, true);
  // WiX 非 zh-CN 会直接 LGHT0311 打包失败
  assert.equal(conf.tauri.bundle.windows.wix.language, 'zh-CN');
  assert.ok(conf.tauri.bundle.identifier, 'bundle.identifier 不能为空');
  const targets = conf.tauri.bundle.targets;
  const list = Array.isArray(targets) ? targets : [targets];
  assert.ok(list.length > 0, 'bundle.targets 不能为空');
});

test('根脚本覆盖 开发 / 构建 / 门禁 / 发版', () => {
  for (const key of ['tauri', 'dev', 'build:front', 'check', 'release']) {
    assert.equal(typeof rootPkg.scripts[key], 'string', `缺 npm script: ${key}`);
  }
  assert.match(rootPkg.scripts['build:front'], /src-react/);
  for (const key of ['check', 'release']) {
    const m = rootPkg.scripts[key].match(/scripts[\\/]([\w.-]+\.ps1)/);
    assert.ok(m, `${key} 应调用 scripts/*.ps1，实际：${rootPkg.scripts[key]}`);
    assert.ok(exists(`scripts/${m[1]}`), `脚本不存在：scripts/${m[1]}`);
  }
});

test('打包脚本与锁文件齐备', () => {
  for (const p of ['scripts/check.ps1', 'scripts/release.ps1', 'scripts/version.mjs']) {
    assert.ok(exists(p), `缺文件：${p}`);
  }
  // 锁文件是「同一个提交构建出同一个包」的前提
  for (const p of ['package-lock.json', 'src-react/package-lock.json', 'src-tauri/Cargo.lock']) {
    assert.ok(exists(p), `缺锁文件：${p}`);
  }
});

test('Rust 清单：MSRV 与 release 优化档在位', () => {
  const msrv = cargoPackageField('rust-version');
  assert.ok(msrv, 'Cargo.toml 应声明 rust-version（Tauri 1.8 需 >= 1.70）');
  const [major, minor] = msrv.split('.').map(Number);
  assert.ok(major > 1 || (major === 1 && minor >= 70), `rust-version=${msrv} 低于依赖要求 1.70`);
  assert.match(cargoToml, /^\[profile\.release\]/m, '缺 [profile.release]：发行包应开 LTO/strip 控制体积');
  assert.match(cargoToml, /^strip\s*=\s*true/m);
  assert.match(cargoToml, /^tauri\s*=\s*\{[^}]*version\s*=\s*"1\./m, 'tauri 应留在 v1 线');
});

test('前端依赖不许回退到废弃 / 无用声明', () => {
  assert.equal(rootPkg.private, true, '私有应用不应被误发布');
  assert.equal(rootPkg.engines.node, '>=22.12');
  assert.equal(frontPkg.engines.node, '>=22.12');
  // dompurify 3.x 自带类型，@types/dompurify 是废弃 stub，装了会遮蔽真实类型
  assert.ok(!frontPkg.devDependencies['@types/dompurify'], '@types/dompurify 已废弃，删掉');
});

test('.gitignore 收住构建与本地缓存产物', () => {
  const ignore = read('.gitignore');
  for (const entry of ['artifacts', '.npm-cache', 'logs']) {
    assert.ok(ignore.includes(entry), `.gitignore 应忽略 ${entry}`);
  }
});

test('预发布版本的 MSI 限制有脚本兜底', () => {
  // WiX 只接受纯数字预发布标识：0.0.2-rc.1 会让 msi bundler 直接失败。
  // 这段不是无用代码，删掉就会在发版当天炸，所以钉住。
  const release = read('scripts/release.ps1');
  assert.match(release, /nonNumericPre/, 'release.ps1 缺预发布标识判定');
  assert.match(release, /droppedTargets/, 'build-manifest.json 应记录被剔除的产物目标');
  assert.match(release, /numeric-only/, '保留 WiX 限制的注释，说明为何剔除 msi');
});

test('门禁脚本必须带 UTF-8 BOM', () => {
  // 2026-11-15 真实事故：编辑器重写 check.ps1 时静默剥掉 BOM，release.ps1 用
  // powershell.exe（5.1）按 GBK 解码中文，把「…未真正校验"」的收尾引号当成双字节
  // 尾字节吃掉，字符串一路吞到下一行 ⇒ 12 条语法错误，打包在门禁处 exit 1。
  for (const p of ['scripts/check.ps1', 'scripts/release.ps1']) {
    const buf = readFileSync(join(root, p));
    assert.ok(
      buf[0] === 0xef && buf[1] === 0xbb && buf[2] === 0xbf,
      `${p} 丢了 UTF-8 BOM：PowerShell 5.1 会按 GBK 读中文，门禁会语法崩`,
    );
  }
});
