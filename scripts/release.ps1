#Requires -Version 5.1
<#
.SYNOPSIS
  Kettd 一键发布打包：版本同步 → 门禁 → 前端构建 → Tauri 打包 → 产物归档。

.DESCRIPTION
  把「发一个版本」固定成一条可复现流水线：
    1. 前置检查：node / npm / cargo / rustc / Tauri CLI 是否就位，前端依赖是否装过
    2. 版本：-Version 传入则写入四处清单（package.json / src-react / tauri.conf / Cargo.toml），
       随后强校验统一；不传则只校验，不一致直接失败
    3. 门禁：默认跑 scripts/check.ps1（-SkipChecks 可跳过，仅限本地救火）
    4. 打包：tauri build，产物目标由 -Bundles 或 tauri.conf 的 bundle.targets 决定；
       预发布版本（如 0.0.2-rc.1）的版本标识不是纯数字，MSI(WiX) 不支持，脚本自动只打 NSIS，
       并在 build-manifest.json 的 droppedTargets 里记下被剔除的目标；
       bundle 阶段遇到 Defender 瞬时锁（os error 32 / 文件被占用）自动退避重试
    5. 校验与归档：确认每个目标都真的产出安装包，复制到 artifacts/v<版本>/，
       附 SHA256SUMS.txt 与 build-manifest.json（工具链版本、git 提交、耗时、体积）
  日志落在 logs/release-<时间戳>.log。

.PARAMETER Version
  目标版本号，如 2.3.0。给了就先同步四处清单再打包，省掉手工改四处版本。

.PARAMETER Bundles
  逗号分隔的产物目标：msi,nsis。缺省读 tauri.conf.json 的 bundle.targets（all = msi + nsis）。
  版本是预发布且标识含非数字（如 0.0.2-rc.1）时，msi 会被自动剔除，只留 nsis。

.PARAMETER SkipChecks
  跳过门禁。正式发版不要用，只用于「刚跑过 check 又要重打包」的场景。

.PARAMETER Clean
  打包前 cargo clean（全量重编译，用于排除增量缓存问题，耗时明显增加）。

.PARAMETER NoArchive
  只打包，不复制到 artifacts/，也不生成校验和与清单。

.PARAMETER Retries
  bundle 阶段瞬时失败的重试次数，默认 2（合计最多 3 次尝试）。

.EXAMPLE
  npm run release

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File scripts/release.ps1 -Version 2.3.0

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File scripts/release.ps1 -Bundles nsis -SkipChecks
#>
[CmdletBinding()]
param(
  [string]$Version,
  [string]$Bundles,
  [switch]$SkipChecks,
  [switch]$Clean,
  [switch]$NoArchive,
  [int]$Retries = 2
)

$ErrorActionPreference = 'Continue'

$Root = Split-Path -Parent $PSScriptRoot
$FrontDir = Join-Path $Root 'src-react'
$TauriDir = Join-Path $Root 'src-tauri'
$TauriConf = Join-Path $TauriDir 'tauri.conf.json'
$TauriCli = Join-Path $Root 'node_modules\.bin\tauri.cmd'

$LogDir = Join-Path $Root 'logs'
if (-not (Test-Path $LogDir)) { New-Item -ItemType Directory -Path $LogDir -Force | Out-Null }
$Stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$LogFile = Join-Path $LogDir "release-$Stamp.log"
$script:StartedAt = Get-Date
$script:Timer = [System.Diagnostics.Stopwatch]::StartNew()

# 日志统一 UTF-8 无 BOM：PS 5.1 与 PS 7 的 Tee-Object 默认编码不同（UTF-16 / UTF-8），
# 混用会把日志写成半截乱码，这里自己写文件，两个版本行为一致。
$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)

function Write-LogOnly([string]$Text) {
  [System.IO.File]::AppendAllText($LogFile, $Text + [Environment]::NewLine, $Utf8NoBom)
}

function Write-Both([string]$Text) {
  Write-Host $Text
  [System.IO.File]::AppendAllText($LogFile, $Text + [Environment]::NewLine, $Utf8NoBom)
}

# 原生命令的 stderr 行会被 PS 5.1 包成 ErrorRecord，直接 "$_" 只会得到
# "System.Management.Automation.RemoteException"，必须取 Exception.Message。
function Convert-Line($Item) {
  if ($Item -is [System.Management.Automation.ErrorRecord]) { return $Item.Exception.Message }
  return [string]$Item
}

function Write-Head([string]$Text) {
  Write-Host ''
  Write-Host "== $Text" -ForegroundColor Cyan
  Write-LogOnly "`n### $Text"
}

function Write-Detail([string]$Text) {
  Write-Host "   $Text" -ForegroundColor DarkGray
  Write-LogOnly "   $Text"
}

function Stop-Fail([string]$Text) {
  Write-Host ''
  Write-Host "[FAIL] $Text" -ForegroundColor Red
  Write-Host "       日志：$LogFile" -ForegroundColor DarkGray
  Write-LogOnly "[FAIL] $Text"
  exit 1
}

# 跑外部命令：stderr 并入日志与屏幕，返回退出码
function Invoke-Native {
  param(
    [Parameter(Mandatory = $true)][string]$FilePath,
    [string[]]$Arguments = @(),
    [string]$WorkDir = $Root
  )
  Push-Location $WorkDir
  try {
    & $FilePath @Arguments 2>&1 | ForEach-Object { Write-Both (Convert-Line $_) }
    return $LASTEXITCODE
  }
  finally {
    Pop-Location
  }
}

# 静默取一行命令输出（失败返回空串，不写日志）
function Get-CommandText {
  param([string]$FilePath, [string[]]$Arguments = @(), [string]$WorkDir = $Root)
  try {
    Push-Location $WorkDir
    $text = (& $FilePath @Arguments 2>$null | Select-Object -First 1)
    if ($null -eq $text) { return '' }
    return "$text".Trim()
  }
  catch {
    return ''
  }
  finally {
    Pop-Location
  }
}

Write-Host "Kettd 发布打包  $Stamp" -ForegroundColor Cyan
Write-Host "仓库：$Root"
Write-Host "日志：$LogFile" -ForegroundColor DarkGray

# ── 1. 前置检查 ───────────────────────────────────────────────
Write-Head '前置检查'

$node = Get-Command node.exe -ErrorAction SilentlyContinue
if (-not $node) { Stop-Fail '找不到 node.exe，请安装 Node.js >= 22.12 并加入 PATH' }
$npm = Get-Command npm.cmd -ErrorAction SilentlyContinue
if (-not $npm) { $npm = Get-Command npm -ErrorAction SilentlyContinue }
if (-not $npm) { Stop-Fail '找不到 npm，请安装 Node.js >= 22.12 并加入 PATH' }
$cargo = Get-Command cargo.exe -ErrorAction SilentlyContinue
if (-not $cargo) { Stop-Fail '找不到 cargo.exe，请安装 Rust stable（MSVC 工具链）' }
if (-not (Test-Path $TauriCli)) {
  Stop-Fail '根目录缺 node_modules（Tauri CLI）：先执行 npm install'
}
if (-not (Test-Path (Join-Path $FrontDir 'node_modules'))) {
  Stop-Fail 'src-react 缺 node_modules：先执行 cd src-react && npm install'
}

$nodeVer = Get-CommandText $node.Source @('-v')
$npmVer = Get-CommandText $npm.Source @('-v')
$cargoVer = Get-CommandText $cargo.Source @('-V')
$rustcVer = Get-CommandText 'rustc.exe' @('-V')
Write-Detail "node $nodeVer / npm $npmVer"
Write-Detail "$cargoVer / $rustcVer"

# ── 2. 版本 ───────────────────────────────────────────────────
Write-Head '版本'
if ($Version) {
  Write-Detail "写入版本：$Version"
  $setExit = Invoke-Native -FilePath $node.Source -Arguments @('scripts/version.mjs', 'set', $Version)
  if ($setExit -ne 0) { Stop-Fail "版本写入失败（exit $setExit）" }
}
$checkExit = Invoke-Native -FilePath $node.Source -Arguments @('scripts/version.mjs', 'check')
if ($checkExit -ne 0) {
  Stop-Fail '四个清单版本不一致：用 -Version <x.y.z> 统一，或手工改齐后重跑'
}
$ProductVersion = (Get-CommandText $node.Source @('scripts/version.mjs', 'get'))
if (-not $ProductVersion) { Stop-Fail '读不到产品版本（scripts/version.mjs get）' }
Write-Detail "产品版本：$ProductVersion"

# ── 3. 门禁 ───────────────────────────────────────────────────
if ($SkipChecks) {
  Write-Head '门禁（已跳过）'
  Write-Host '   ! -SkipChecks：本次产物未经过门禁，勿用于正式发版' -ForegroundColor Yellow
}
else {
  Write-Head '门禁'
  $gateArgs = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', (Join-Path $PSScriptRoot 'check.ps1'))
  $gateExit = Invoke-Native -FilePath 'powershell.exe' -Arguments $gateArgs
  if ($gateExit -ne 0) { Stop-Fail "门禁未通过（exit $gateExit）" }
}

# ── 4. 打包 ───────────────────────────────────────────────────
Write-Head '打包'

if ($Clean) {
  Write-Detail 'cargo clean（全量重编译）'
  $cleanExit = Invoke-Native -FilePath $cargo.Source -Arguments @('clean') -WorkDir $TauriDir
  if ($cleanExit -ne 0) { Stop-Fail "cargo clean 失败（exit $cleanExit）" }
}

# 产物目标：命令行 > tauri.conf.json；all 在 Windows 上等价于 msi + nsis
$conf = Get-Content $TauriConf -Raw -Encoding UTF8 | ConvertFrom-Json
$configured = @()
$rawTargets = $conf.tauri.bundle.targets
if ($rawTargets -is [string]) {
  if ($rawTargets -eq 'all') { $configured = @('msi', 'nsis') } else { $configured = @($rawTargets) }
}
elseif ($rawTargets) { $configured = @($rawTargets) }

if ($Bundles) {
  $targets = @($Bundles.Split(',') | ForEach-Object { $_.Trim().ToLowerInvariant() } | Where-Object { $_ })
}
elseif ($configured.Count -gt 0) {
  $targets = $configured
}
else {
  $targets = @('msi', 'nsis')
}

$supported = @('msi', 'nsis')
$unknown = @($targets | Where-Object { $supported -notcontains $_ })
if ($unknown.Count -gt 0) { Stop-Fail "Windows 下不支持的产物目标：$($unknown -join ', ')（可用：msi, nsis）" }

# MSI(WiX) 只接受纯数字的预发布标识：0.0.2-rc.1 会被 bundler 直接拒绝
# （optional pre-release identifier in app version must be numeric-only ... for msi target）。
# 这里自动把 msi 收窄掉，只出 NSIS，并在 build-manifest.json 里记下 droppedTargets。
$preTag = ''
if ($ProductVersion -match '-([0-9A-Za-z.-]+)$') { $preTag = $Matches[1] }
$nonNumericPre = $false
if ($preTag) {
  $ids = @($preTag.Split('.') | Where-Object { $_ })
  $nonNumericPre = @($ids | Where-Object { $_ -notmatch '^\d+$' }).Count -gt 0
}
$script:DroppedTargets = @()
if ($nonNumericPre -and ($targets -contains 'msi')) {
  $script:DroppedTargets = @('msi')
  $targets = @($targets | Where-Object { $_ -ne 'msi' })
  Write-Host "   ! 版本 $ProductVersion 的预发布标识「$preTag」不是纯数字，MSI(WiX) 不支持，本次只打 NSIS" -ForegroundColor Yellow
  Write-LogOnly "   收窄产物目标：剔除 msi（版本 $ProductVersion 的预发布标识 $preTag 非纯数字，WiX 要求 numeric-only）"
  if ($targets.Count -eq 0) {
    Stop-Fail "版本 $ProductVersion 出不了 MSI：预发布标识必须是纯数字（如 0.0.2-1），或改用 -Bundles nsis"
  }
}
Write-Detail "产物目标：$($targets -join ', ')"

$buildArgs = @('build', '--bundles', ($targets -join ','))
$maxAttempts = [Math]::Max(1, $Retries + 1)
$buildExit = 1
for ($attempt = 1; $attempt -le $maxAttempts; $attempt++) {
  Write-Detail "tauri $($buildArgs -join ' ')（第 $attempt/$maxAttempts 次）"
  $buildExit = Invoke-Native -FilePath $TauriCli -Arguments $buildArgs
  if ($buildExit -eq 0) { break }

  $tail = ''
  if (Test-Path $LogFile) { $tail = (Get-Content $LogFile -Tail 60 -Encoding UTF8) -join "`n" }
  # Defender 扫描窗口造成的瞬时占用：等一会儿再试通常就过
  if ($attempt -lt $maxAttempts -and $tail -match 'os error 32|error 32|being used by another process|另一个程序正在使用|拒绝访问|Access is denied') {
    $wait = 5 * $attempt
    Write-Host "   ! 疑似瞬时文件占用（os error 32 一类），${wait}s 后重试" -ForegroundColor Yellow
    Start-Sleep -Seconds $wait
    continue
  }
  break
}
if ($buildExit -ne 0) { Stop-Fail "tauri build 失败（exit $buildExit，共尝试 $([Math]::Min($attempt, $maxAttempts)) 次）" }

# ── 5. 产物校验 ───────────────────────────────────────────────
Write-Head '产物校验'
$bundleDir = Join-Path $TauriDir 'target\release\bundle'
if (-not (Test-Path $bundleDir)) { Stop-Fail "找不到 bundle 目录：$bundleDir" }

$artifacts = New-Object System.Collections.ArrayList
foreach ($target in $targets) {
  $dir = $null
  $pattern = $null
  switch ($target) {
    'msi' { $dir = Join-Path $bundleDir 'msi'; $pattern = '*.msi' }
    'nsis' { $dir = Join-Path $bundleDir 'nsis'; $pattern = '*.exe' }
  }
  if (-not $dir -or -not (Test-Path $dir)) { Stop-Fail "缺少 $target 产物目录：$dir" }

  $candidates = @(Get-ChildItem -Path $dir -Filter $pattern -File -ErrorAction SilentlyContinue)
  if ($candidates.Count -eq 0) { Stop-Fail "bundle 报告成功，但 $dir 里没有 $pattern" }

  # 优先取文件名带当前版本的（避免旧版本安装包混进来），否则取最新
  $matched = @($candidates | Where-Object { $_.Name -like "*$ProductVersion*" })
  if ($matched.Count -gt 0) {
    $file = $matched | Sort-Object LastWriteTime -Descending | Select-Object -First 1
  }
  else {
    $file = $candidates | Sort-Object LastWriteTime -Descending | Select-Object -First 1
    Write-Host "   ! $target 产物文件名不含 $ProductVersion，取最新：$($file.Name)" -ForegroundColor Yellow
  }
  [void]$artifacts.Add([pscustomobject]@{ Target = $target; File = $file })
  Write-Detail "$target → $($file.Name)  $([math]::Round($file.Length / 1MB, 2)) MB"
}

# ── 6. 归档 ───────────────────────────────────────────────────
$outDir = Join-Path $Root "artifacts\v$ProductVersion"
if ($NoArchive) {
  Write-Head '归档（已跳过 -NoArchive）'
}
else {
  Write-Head "归档 → $outDir"
  if (-not (Test-Path $outDir)) { New-Item -ItemType Directory -Path $outDir -Force | Out-Null }

  $sumLines = New-Object System.Collections.ArrayList
  $manifestFiles = New-Object System.Collections.ArrayList
  foreach ($a in $artifacts) {
    $dest = Join-Path $outDir $a.File.Name
    Copy-Item -Path $a.File.FullName -Destination $dest -Force
    $hash = (Get-FileHash -Path $dest -Algorithm SHA256).Hash.ToLowerInvariant()
    [void]$sumLines.Add("$hash  $($a.File.Name)")
    [void]$manifestFiles.Add([pscustomobject]@{
        target    = $a.Target
        file      = $a.File.Name
        sizeBytes = (Get-Item $dest).Length
        sha256    = $hash
      })
    Write-Detail "$($a.File.Name)  sha256=$($hash.Substring(0, 12))…"
  }
  Set-Content -Path (Join-Path $outDir 'SHA256SUMS.txt') -Value $sumLines -Encoding ASCII

  $gitCommit = Get-CommandText 'git.exe' @('rev-parse', '--short', 'HEAD')
  $gitBranch = Get-CommandText 'git.exe' @('rev-parse', '--abbrev-ref', 'HEAD')
  $gitDirty = ((Get-CommandText 'git.exe' @('status', '--porcelain')) -ne '')

  $script:Timer.Stop()
  $manifest = [pscustomobject]@{
    product         = 'Kettd'
    version         = $ProductVersion
    builtAt         = $script:StartedAt.ToString('o')
    durationSeconds = [math]::Round($script:Timer.Elapsed.TotalSeconds, 1)
    git             = [pscustomobject]@{ commit = $gitCommit; branch = $gitBranch; dirty = $gitDirty }
    toolchain       = [pscustomobject]@{ node = $nodeVer; npm = $npmVer; cargo = $cargoVer; rustc = $rustcVer }
    bundles         = $manifestFiles
    checks          = $(if ($SkipChecks) { 'skipped' } else { 'passed' })
    cleanBuild      = [bool]$Clean
    droppedTargets  = $script:DroppedTargets
    log             = $LogFile
  }
  $manifest | ConvertTo-Json -Depth 6 | Set-Content -Path (Join-Path $outDir 'build-manifest.json') -Encoding UTF8
  Write-Detail '已生成 SHA256SUMS.txt 与 build-manifest.json'
  if ($gitDirty) {
    Write-Host "   ! 工作区有未提交改动，清单里 git.dirty=true（正式发版应基于干净提交）" -ForegroundColor Yellow
  }
}

# ── 汇总 ──────────────────────────────────────────────────────
$script:Timer.Stop()
Write-Host ''
Write-Host "== 完成  Kettd $ProductVersion  用时 $([math]::Round($script:Timer.Elapsed.TotalSeconds, 1))s" -ForegroundColor Green
$artifacts | ForEach-Object {
  Write-Host ("   {0,-5} {1}" -f $_.Target, $_.File.FullName)
}
if (-not $NoArchive) { Write-Host "   归档  $outDir" }
Write-Host "   日志  $LogFile" -ForegroundColor DarkGray
exit 0
