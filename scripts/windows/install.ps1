<#
.SYNOPSIS
  KoeType を Windows ユーザー環境へ配置する

.DESCRIPTION
  `pnpm tauri build` で生成された exe を
  `%LOCALAPPDATA%\\KoeType\\KoeType.exe` にコピーします。
  必要に応じてスタートアップ登録と即時起動を行います。
#>

[CmdletBinding()]
param(
  [string]$SourceExe,
  [string]$InstallDir = (Join-Path $env:LOCALAPPDATA 'KoeType'),
  [string]$AppExeName = 'KoeType.exe',
  [switch]$RegisterStartup,
  [switch]$StartNow
)

$ErrorActionPreference = 'Stop'

function Resolve-DefaultSourceExe {
  $repoRoot = Resolve-Path (Join-Path $PSScriptRoot '..\..')
  $candidates = @(
    (Join-Path $repoRoot 'src-tauri\target\release\koetype.exe')
  )
  foreach ($candidate in $candidates) {
    if (Test-Path -LiteralPath $candidate) {
      return $candidate
    }
  }
  return $candidates[0]
}

function Ensure-FileExists([string]$PathToCheck, [string]$Label) {
  if (-not (Test-Path -LiteralPath $PathToCheck)) {
    throw ($Label + ' が見つかりません: ' + $PathToCheck + '。先に pnpm tauri build を実行してください。')
  }
}

function Register-StartupShortcut([string]$TargetExePath) {
  $startupDir = [Environment]::GetFolderPath('Startup')
  $lnkPath = Join-Path $startupDir 'KoeType.lnk'

  $ws = New-Object -ComObject WScript.Shell
  $sc = $ws.CreateShortcut($lnkPath)
  $sc.TargetPath = $TargetExePath
  $sc.WorkingDirectory = (Split-Path -Parent $TargetExePath)
  $sc.Description = 'KoeType をスタートアップで起動'
  $sc.Save()
  return $lnkPath
}

try {
  if (-not $SourceExe) {
    $SourceExe = Resolve-DefaultSourceExe
  }

  Ensure-FileExists -PathToCheck $SourceExe -Label 'ビルド成果物'

  New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
  $destExe = Join-Path $InstallDir $AppExeName
  Copy-Item -LiteralPath $SourceExe -Destination $destExe -Force

  Write-Host '配置完了:'
  Write-Host ('  コピー元: ' + $SourceExe)
  Write-Host ('  コピー先: ' + $destExe)

  if ($RegisterStartup) {
    $lnkPath = Register-StartupShortcut -TargetExePath $destExe
    Write-Host ('スタートアップ登録完了: ' + $lnkPath)
  } else {
    Write-Host 'スタートアップ登録: スキップ（必要なら -RegisterStartup を指定）'
  }

  if ($StartNow) {
    Start-Process -FilePath $destExe
    Write-Host 'アプリを起動しました。'
  } else {
    Write-Host '即時起動: スキップ（必要なら -StartNow を指定）'
  }
}
catch {
  Write-Error ('配置処理でエラーが発生しました: ' + $_.Exception.Message)
  exit 1
}



