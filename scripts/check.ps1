#Requires -Version 5.1
<#
.SYNOPSIS
  Kettd 门禁聚合：一条命令跑完「提交前必过项」。

.DESCRIPTION
  顺序执行（快 → 慢，任一项失败即停）：
    1. 版本一致性（package.json / src-react / tauri.conf / Cargo.toml 四处对齐）
    2. 契约门禁：命令四方对账、主窗口关闭契约、打包配置契约、文档结构契约
    3. 前端门禁：Markdown 渲染保真、Markdown round-trip
    4. 前端：类型检查 → lint → 生产构建
    5. Rust：cargo check → cargo test
  全量日志写入 logs/check-<时间戳>.log，控制台只留摘要；退出码 0=全绿，1=有红。

.PARAMETER SkipRust
  跳过 cargo check / cargo test（仅改前端时省时间）。

.PARAMETER SkipFront
  跳过前端类型检查 / lint / 构建。

.PARAMETER SkipNodeTests
  跳过 node --test 契约与 Markdown 门禁。

.PARAMETER ContinueOnError
  某步失败后继续跑完剩余步骤，最后统一汇总（默认 fail-fast）。

.EXAMPLE
  npm run check

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File scripts/check.ps1 -SkipRust
#>
[CmdletBinding()]
param(
  [switch]$SkipRust,
  [switch]$SkipFront,
  [switch]$SkipNodeTests,
  [switch]$ContinueOnError
)

$ErrorActionPreference = 'Continue'
Set-StrictMode -Version Latest

$Root = Split-Path -Parent $PSScriptRoot
$FrontDir = Join-Path $Root 'src-react'
$TauriDir = Join-Path $Root 'src-tauri'

$LogDir = Join-Path $Root 'logs'
if (-not (Test-Path $LogDir)) { New-Item -ItemType Directory -Path $LogDir -Force | Out-Null }
$Stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$LogFile = Join-Path $LogDir "check-$Stamp.log"
$script:Results = New-Object System.Collections.ArrayList
$script:HasFailure = $false
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
}

# 统一跑外部命令：stderr 也并入日志与屏幕，$LASTEXITCODE 作为结果
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

# 取日志文件从 $From 字节起的增量内容（用于识别本步是否「跳过而非通过」）
function Get-LogDelta {
  param([long]$From)
  if (-not (Test-Path $LogFile)) { return '' }
  $fs = [System.IO.File]::Open($LogFile, 'Open', 'Read', 'ReadWrite')
  try {
    $pos = [Math]::Min($From, $fs.Length)
    $fs.Position = $pos
    $reader = New-Object System.IO.StreamReader($fs, [System.Text.Encoding]::UTF8)
    return $reader.ReadToEnd()
  }
  finally {
    $fs.Dispose()
  }
}

function Invoke-Step {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [Parameter(Mandatory = $true)][string]$FilePath,
    [string[]]$Arguments = @(),
    [string]$WorkDir = $Root
  )

  $logFrom = 0
  if (Test-Path $LogFile) { $logFrom = (Get-Item $LogFile).Length }

  Write-Head $Name
  "`n### $Name :: $FilePath $($Arguments -join ' ') (cwd=$WorkDir)" | Add-Content -Path $LogFile -Encoding UTF8

  $timer = [System.Diagnostics.Stopwatch]::StartNew()
  $exitCode = 1
  try {
    $exitCode = Invoke-Native -FilePath $FilePath -Arguments $Arguments -WorkDir $WorkDir
    if ($null -eq $exitCode) { $exitCode = 0 }
  }
  catch {
    Write-Host ("  ! " + $_.Exception.Message) -ForegroundColor Red
    $exitCode = 1
  }
  finally {
    $timer.Stop()
  }

  # 门禁「静默跳过」比红更危险：例如 md-roundtrip 缺 jsdom 时会打印 [SKIP] 并 exit 0
  $skipped = $false
  if ($exitCode -eq 0) {
    $delta = Get-LogDelta -From $logFrom
    $skipped = ($delta -match '\[SKIP\]')
  }

  $ok = ($exitCode -eq 0)
  if (-not $ok) { $script:HasFailure = $true }

  $result = [pscustomobject]@{
    Step     = $Name
    Ok       = $ok
    Skipped  = $skipped
    ExitCode = $exitCode
    Seconds  = [math]::Round($timer.Elapsed.TotalSeconds, 1)
  }
  [void]$script:Results.Add($result)

  if ($skipped) {
    Write-Host ("  [SKIP] {0}  ({1}s) — 环境不满足，未真正校验" -f $Name, $result.Seconds) -ForegroundColor Yellow
  }
  elseif ($ok) {
    Write-Host ("  [OK]   {0}  ({1}s)" -f $Name, $result.Seconds) -ForegroundColor Green
  }
  else {
    Write-Host ("  [FAIL] {0}  (exit {1}, {2}s) — 详见 {3}" -f $Name, $exitCode, $result.Seconds, $LogFile) -ForegroundColor Red
  }
  return $ok
}

# 前置：Node 必须可用（本项目所有门禁都靠它）
$node = Get-Command node.exe -ErrorAction SilentlyContinue
if (-not $node) {
  Write-Host '找不到 node.exe：请先安装 Node.js >= 22.12 并加入 PATH' -ForegroundColor Red
  exit 1
}

$npm = Get-Command npm.cmd -ErrorAction SilentlyContinue
if (-not $npm) { $npm = Get-Command npm -ErrorAction SilentlyContinue }
if (-not $npm) {
  Write-Host '找不到 npm：请先安装 Node.js >= 22.12 并加入 PATH' -ForegroundColor Red
  exit 1
}
$npmExe = $npm.Source

Write-Host "Kettd 门禁检查  $Stamp" -ForegroundColor Cyan
Write-Host "仓库：$Root"
Write-Host "日志：$LogFile" -ForegroundColor DarkGray

# ── 1. 版本一致性 ───────────────────────────────────────────────
Invoke-Step -Name '版本一致性' -FilePath $node.Source -Arguments @('scripts/version.mjs', 'check') | Out-Null

# ── 2~3. Node 门禁 ─────────────────────────────────────────────
if (-not $SkipNodeTests -and (-not $script:HasFailure -or $ContinueOnError)) {
  Invoke-Step -Name '命令四方对账' -FilePath $node.Source -Arguments @('--test', 'test/command-parity.test.mjs') | Out-Null
  if (-not $script:HasFailure -or $ContinueOnError) {
    Invoke-Step -Name '主窗口关闭契约' -FilePath $node.Source -Arguments @('--test', 'test/main-window-close.test.mjs') | Out-Null
  }
  if (-not $script:HasFailure -or $ContinueOnError) {
    Invoke-Step -Name '打包配置契约' -FilePath $node.Source -Arguments @('--test', 'test/packaging.test.mjs') | Out-Null
  }
  if (-not $script:HasFailure -or $ContinueOnError) {
    Invoke-Step -Name '文档结构契约' -FilePath $node.Source -Arguments @('--test', 'test/docs.test.mjs') | Out-Null
  }
  if (-not $script:HasFailure -or $ContinueOnError) {
    Invoke-Step -Name 'Markdown 渲染保真' -FilePath $node.Source -Arguments @('--test', 'src-react/test/md-fidelity.test.mjs') | Out-Null
  }
  if (-not $script:HasFailure -or $ContinueOnError) {
    Invoke-Step -Name 'Markdown round-trip' -FilePath $node.Source -Arguments @('src-react/test/md-roundtrip.test.mjs') | Out-Null
  }
}

# ── 4. 前端 ───────────────────────────────────────────────────
if (-not $SkipFront -and (-not $script:HasFailure -or $ContinueOnError)) {
  Invoke-Step -Name '前端类型检查' -FilePath $npmExe -Arguments @('run', 'typecheck') -WorkDir $FrontDir | Out-Null
  if (-not $script:HasFailure -or $ContinueOnError) {
    Invoke-Step -Name '前端 lint' -FilePath $npmExe -Arguments @('run', 'lint') -WorkDir $FrontDir | Out-Null
  }
  if (-not $script:HasFailure -or $ContinueOnError) {
    Invoke-Step -Name '前端构建' -FilePath $npmExe -Arguments @('run', 'build') -WorkDir $FrontDir | Out-Null
  }
}

# ── 5. Rust ───────────────────────────────────────────────────
if (-not $SkipRust -and (-not $script:HasFailure -or $ContinueOnError)) {
  $cargo = Get-Command cargo.exe -ErrorAction SilentlyContinue
  if (-not $cargo) {
    Write-Host '找不到 cargo.exe：请安装 Rust stable（MSVC）并加入 PATH' -ForegroundColor Red
    $script:HasFailure = $true
  }
  else {
    Invoke-Step -Name 'Rust 编译检查' -FilePath $cargo.Source -Arguments @('check') -WorkDir $TauriDir | Out-Null
    if (-not $script:HasFailure -or $ContinueOnError) {
      Invoke-Step -Name 'Rust 单测' -FilePath $cargo.Source -Arguments @('test') -WorkDir $TauriDir | Out-Null
    }
  }
}

# ── 汇总 ──────────────────────────────────────────────────────
Write-Host ''
Write-Host '== 汇总' -ForegroundColor Cyan
$script:Results | Format-Table -AutoSize -Property @{ Label = '结果'; Expression = { if ($_.Skipped) { 'SKIP' } elseif ($_.Ok) { 'OK' } else { 'FAIL' } } }, Step, ExitCode, @{ Label = '耗时(s)'; Expression = { $_.Seconds } } | Out-Host

$skippedCount = @($script:Results | Where-Object { $_.Skipped }).Count

if ($script:HasFailure) {
  Write-Host '门禁未通过：修完上面的红项再提交。' -ForegroundColor Red
  exit 1
}
if ($skippedCount -gt 0) {
  Write-Host "门禁通过，但有 $skippedCount 项被环境跳过（SKIP），这些门禁本轮没有实际校验。（日志：$LogFile）" -ForegroundColor Yellow
  exit 0
}
Write-Host "门禁全绿，可以提交。（日志：$LogFile）" -ForegroundColor Green
exit 0
