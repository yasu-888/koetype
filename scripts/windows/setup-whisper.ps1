#Requires -Version 5.1
<#
.SYNOPSIS
    KoeType 用 whisper-cli.exe とモデルファイルのセットアップ (Windows)

.DESCRIPTION
    whisper.cpp の Windows 向けバイナリを GitHub Releases からダウンロードし
    {KOETYPE_HOME}/bin/ に配置します。
    モデルファイルは HuggingFace から {KOETYPE_HOME}/models/whisper/ にダウンロードします。

.PARAMETER Model
    ダウンロードするモデル名 (例: large-v3-turbo)。省略時は対話選択。

.PARAMETER Variant
    モデルのバリアント (例: q5_0)。省略時は対話選択。

.PARAMETER Backend
    GPU バックエンド: cpu / cuda11 / cuda12。省略時は対話選択。

.PARAMETER NonInteractive
    対話入力を行わずに実行します。Model / Variant / Backend を必ず指定してください。

.EXAMPLE
    .\scripts\windows\setup-whisper.ps1
    .\scripts\windows\setup-whisper.ps1 -Model large-v3-turbo -Variant q5_0 -Backend cpu
#>

param(
    [string]$Model = "",
    [string]$Variant = "",
    [string]$Backend = "",
    [switch]$MigrateOnly,
    [switch]$NonInteractive
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# --- 定数 ---
$HF_BASE = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main"
$GH_RELEASES_BASE = "https://github.com/ggml-org/whisper.cpp/releases/download"
$WHISPER_VERSION = "v1.8.3"

$MODELS = @(
    "tiny", "tiny.en",
    "base", "base.en",
    "small", "small.en",
    "medium", "medium.en",
    "large-v1", "large-v2", "large-v3", "large-v3-turbo"
)

$BACKENDS = [ordered]@{
    "cpu"    = "whisper-bin-x64.zip"
    "cuda11" = "whisper-cublas-11.8.0-bin-x64.zip"
    "cuda12" = "whisper-cublas-12.4.0-bin-x64.zip"
}

$BACKEND_LABELS = [ordered]@{
    "cpu"    = "CPU only (GPU なし・全環境で動作)"
    "cuda11" = "CUDA 11.8 (NVIDIA GPU, CUDA 11.x)"
    "cuda12" = "CUDA 12.4 (NVIDIA GPU, CUDA 12.x)"
}

$REQUIRED_RUNTIME_DLLS = @("ggml.dll")

# --- パス解決 ---
function Get-AppHomeDir {
    if ($env:KOETYPE_HOME) {
        return $env:KOETYPE_HOME
    }
    # アプリ本体 (Rust: settings::get_app_home_dir) と同じ既定値に揃える
    # Windows でも ~/.config/koetype を使用する
    return Join-Path $HOME ".config\koetype"
}

$appHome = Get-AppHomeDir
$binDir = Join-Path $appHome "bin"
$modelsDir = if ($env:WHISPER_MODELS_DIR) {
    $env:WHISPER_MODELS_DIR
} elseif ($env:KOETYPE_WHISPER_MODELS_DIR) {
    $env:KOETYPE_WHISPER_MODELS_DIR
} else {
    Join-Path $appHome "models\whisper"
}

function Write-KoeTypeErrorLog {
    param([string]$Message)

    try {
        $logPath = Join-Path (Get-AppHomeDir) "error_log.json"
        $entries = @()
        if (Test-Path $logPath) {
            $raw = Get-Content -LiteralPath $logPath -Raw -ErrorAction SilentlyContinue
            if (-not [string]::IsNullOrWhiteSpace($raw)) {
                $parsed = $raw | ConvertFrom-Json -ErrorAction Stop
                if ($parsed -is [System.Array]) {
                    $entries = @($parsed)
                } else {
                    $entries = @($parsed)
                }
            }
        }

        $now = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
        $entry = [pscustomobject]@{
            id         = [int64]$now
            timestamp  = [int64]$now
            error_type = "UnknownError"
            message    = $Message
            context    = "whisper_install_ps1"
        }

        $updated = @($entry) + $entries
        if ($updated.Count -gt 500) {
            $updated = $updated[0..499]
        }

        $logDir = Split-Path -Parent $logPath
        if (-not [string]::IsNullOrWhiteSpace($logDir)) {
            New-Item -ItemType Directory -Force -Path $logDir | Out-Null
        }

        $updated | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $logPath -Encoding UTF8
    } catch {
        Write-Warning "error_log.json への書き込みに失敗しました: $($_.Exception.Message)"
    }
}

trap {
    $base = $_.Exception.Message
    $position = $_.InvocationInfo.PositionMessage
    $fullMessage = if ([string]::IsNullOrWhiteSpace($position)) {
        "Whisperインストール実行失敗: $base"
    } else {
        "Whisperインストール実行失敗: $base`n$position"
    }

    Write-Host ""
    Write-Error $fullMessage
    Write-KoeTypeErrorLog -Message $fullMessage
    Write-Host ""
    Read-Host "Enterキーで閉じる" | Out-Null
    exit 1
}

function Get-LegacyAppHomes {
    $localAppData = [System.Environment]::GetFolderPath("LocalApplicationData")
    @(
        (Join-Path $localAppData "koetype"),
        (Join-Path $localAppData "KoeType"),
        (Join-Path $localAppData "AquaClone"),
        (Join-Path $localAppData "aquaclone"),
        (Join-Path $HOME ".config\aquaclone"),
        (Join-Path $HOME ".config\AquaClone")
    )
}

function Migrate-LegacyWhisperArtifacts {
    param(
        [string]$CurrentBinDir,
        [string]$CurrentModelsDir
    )

    if (Test-Path (Join-Path $CurrentBinDir "whisper-cli.exe")) {
        return
    }

    foreach ($legacyHome in (Get-LegacyAppHomes)) {
        $legacyBin = Join-Path $legacyHome "bin"
        $legacyCli = Join-Path $legacyBin "whisper-cli.exe"
        if (-not (Test-Path $legacyCli)) {
            continue
        }

        New-Item -ItemType Directory -Force -Path $CurrentBinDir | Out-Null
        Copy-Item $legacyCli (Join-Path $CurrentBinDir "whisper-cli.exe") -Force

        Get-ChildItem -Path $legacyBin -Filter "*.dll" -File -ErrorAction SilentlyContinue |
            ForEach-Object { Copy-Item $_.FullName $CurrentBinDir -Force }

        $legacyModelsDir = Join-Path $legacyHome "models\whisper"
        if (Test-Path $legacyModelsDir) {
            New-Item -ItemType Directory -Force -Path $CurrentModelsDir | Out-Null
            Get-ChildItem -Path $legacyModelsDir -Filter "ggml-*.bin" -File -ErrorAction SilentlyContinue |
                ForEach-Object {
                    $dest = Join-Path $CurrentModelsDir $_.Name
                    if (-not (Test-Path $dest)) {
                        Copy-Item $_.FullName $dest -Force
                    }
                }
        }

        Write-Host "旧ディレクトリから whisper 関連ファイルを移行しました: $legacyHome -> $appHome"
        return
    }
}

Migrate-LegacyWhisperArtifacts -CurrentBinDir $binDir -CurrentModelsDir $modelsDir

if ($MigrateOnly) {
    $migratedCliPath = Join-Path $binDir "whisper-cli.exe"
    Write-Host ""
    Write-Host "移行モードで実行しました。"
    Write-Host "  app_home:    $appHome"
    Write-Host "  whisper-cli: $migratedCliPath"
    Write-Host "  models_dir:  $modelsDir"
    exit 0
}

# --- ユーティリティ ---
function Select-FromList {
    param([string]$Prompt, [string[]]$Items)
    for ($i = 0; $i -lt $Items.Count; $i++) {
        Write-Host ("  {0,2}) {1}" -f ($i + 1), $Items[$i])
    }
    $choice = Read-Host "$Prompt [1-$($Items.Count)]"
    if ($choice -match '^\d+$' -and [int]$choice -ge 1 -and [int]$choice -le $Items.Count) {
        return $Items[[int]$choice - 1]
    }
    Write-Error "無効な選択です: $choice"
    exit 2
}

function Get-Variants {
    param([string]$ModelName)
    switch -Regex ($ModelName) {
        '^(tiny|tiny\.en|base|base\.en|small|small\.en)$' { return @("full", "q5_1", "q8_0") }
        '^(medium|medium\.en|large-v2|large-v3|large-v3-turbo)$' { return @("full", "q5_0", "q8_0") }
        '^large-v1$' { return @("full") }
        default {
            Write-Error "不明なモデル: $ModelName"
            exit 2
        }
    }
}

function Get-ModelFileName {
    param([string]$ModelName, [string]$VariantName)
    if ($VariantName -eq "full") {
        return "ggml-${ModelName}.bin"
    } else {
        return "ggml-${ModelName}-${VariantName}.bin"
    }
}

function Invoke-Download {
    param([string]$Url, [string]$Dest, [string]$Label)
    Write-Host "ダウンロード中: $Label"
    Write-Host "  URL:  $Url"
    Write-Host "  保存先: $Dest"
    try {
        $webClient = New-Object System.Net.WebClient
        $webClient.DownloadFile($Url, $Dest)
    } catch {
        Write-Error "ダウンロード失敗: $_"
        exit 1
    }
    Write-Host "完了: $Dest"
}

# --- Step 1: whisper-cli.exe のセットアップ ---
Write-Host ""
Write-Host "=== whisper-cli.exe セットアップ ==="
Write-Host ""

$skipBinary = $false
$cliDest = Join-Path $binDir "whisper-cli.exe"
if (Test-Path $cliDest) {
    Write-Host "既に存在します: $cliDest"
    $missingDlls = @()
    foreach ($dllName in $REQUIRED_RUNTIME_DLLS) {
        if (-not (Test-Path (Join-Path $binDir $dllName))) {
            $missingDlls += $dllName
        }
    }

    if ($missingDlls.Count -gt 0) {
        Write-Warning "ランタイム DLL が不足しています: $($missingDlls -join ', ')"
        Write-Host "不足ファイルを解消するため、バイナリを再ダウンロードします。"
    } elseif ($NonInteractive) {
        Write-Host "非対話モードのため、既存バイナリを再利用します。"
        $skipBinary = $true
    } else {
        $skip = Read-Host "再ダウンロードしますか？ [y/N]"
        if ($skip -notmatch '^[yY]') {
            Write-Host "バイナリのダウンロードをスキップします。"
            $skipBinary = $true
        }
    }
}

if (-not $skipBinary) {
    # Backend 選択
    if (-not $Backend) {
        if ($NonInteractive) {
            Write-Error "非対話モードでは -Backend を指定してください。"
            exit 2
        }
        Write-Host "GPU バックエンドを選択してください:"
        $backendKeys = @($BACKENDS.Keys)
        for ($i = 0; $i -lt $backendKeys.Count; $i++) {
            Write-Host ("  {0,2}) {1}" -f ($i + 1), $BACKEND_LABELS[$backendKeys[$i]])
        }
        $choice = Read-Host "Backend [1-$($backendKeys.Count)]"
        if ($choice -match '^\d+$' -and [int]$choice -ge 1 -and [int]$choice -le $backendKeys.Count) {
            $Backend = $backendKeys[[int]$choice - 1]
        } else {
            Write-Error "無効な選択です: $choice"
            exit 2
        }
    }

    if (-not $BACKENDS.Contains($Backend)) {
        Write-Error "不明なバックエンド: $Backend (cpu / cuda11 / cuda12)"
        exit 2
    }

    $zipName = $BACKENDS[$Backend]
    $zipUrl = "$GH_RELEASES_BASE/$WHISPER_VERSION/$zipName"
    $zipTmp = Join-Path $env:TEMP "KOETYPE_whisper_$zipName"

    New-Item -ItemType Directory -Force -Path $binDir | Out-Null

    Invoke-Download -Url $zipUrl -Dest $zipTmp -Label "whisper-cli ($Backend)"

    Write-Host "展開中..."
    $extractTmp = Join-Path $env:TEMP "KOETYPE_whisper_extract"
    if (Test-Path $extractTmp) { Remove-Item $extractTmp -Recurse -Force }
    Expand-Archive -Path $zipTmp -DestinationPath $extractTmp -Force

    # zip 内は Release/whisper-cli.exe
    $exeSrc = Join-Path $extractTmp "Release\whisper-cli.exe"
    if (-not (Test-Path $exeSrc)) {
        Write-Error "zip 内に whisper-cli.exe が見つかりません。zip 構造が変わった可能性があります: $extractTmp"
        exit 1
    }

    Copy-Item $exeSrc $cliDest -Force

    # backend に関わらず Runtime DLL をコピー
    $releaseDir = Join-Path $extractTmp "Release"
    $dlls = @(Get-ChildItem -Path $releaseDir -Filter "*.dll" -File)
    if ($dlls.Count -eq 0) {
        Write-Error "zip 内に DLL が見つかりません。zip 構造が変わった可能性があります: $releaseDir"
        exit 1
    }
    foreach ($dll in $dlls) {
        Copy-Item $dll.FullName $binDir -Force
    }

    $stillMissing = @()
    foreach ($dllName in $REQUIRED_RUNTIME_DLLS) {
        if (-not (Test-Path (Join-Path $binDir $dllName))) {
            $stillMissing += $dllName
        }
    }
    if ($stillMissing.Count -gt 0) {
        Write-Error "DLL コピー後も不足しています: $($stillMissing -join ', ')"
        exit 1
    }
    Write-Host "Runtime DLL を $binDir にコピーしました。"

    Remove-Item $zipTmp -Force -ErrorAction SilentlyContinue
    Remove-Item $extractTmp -Recurse -Force -ErrorAction SilentlyContinue

    Write-Host ""
    Write-Host "whisper-cli.exe を配置しました: $cliDest"
}

# --- Step 2: モデルファイルのダウンロード ---
Write-Host ""
Write-Host "=== モデルファイルのダウンロード ==="
Write-Host ""

# モデル選択
if (-not $Model) {
    if ($NonInteractive) {
        Write-Error "非対話モードでは -Model を指定してください。"
        exit 2
    }
    Write-Host "ダウンロードするモデルを選択してください:"
    $Model = Select-FromList -Prompt "Model" -Items $MODELS
}

if ($MODELS -notcontains $Model) {
    Write-Error "不明なモデル: $Model"
    exit 2
}

# バリアント選択
$validVariants = Get-Variants -ModelName $Model
if (-not $Variant) {
    if ($NonInteractive) {
        Write-Error "非対話モードでは -Variant を指定してください。"
        exit 2
    }
    if ($validVariants.Count -eq 1) {
        $Variant = $validVariants[0]
        Write-Host "$Model のバリアントは 1 種類のみです: $Variant"
    } else {
        Write-Host ""
        Write-Host "$Model のバリアントを選択してください:"
        $Variant = Select-FromList -Prompt "Variant" -Items $validVariants
    }
} else {
    if ($validVariants -notcontains $Variant) {
        Write-Error "バリアント '$Variant' は '$Model' に対して無効です。利用可能: $($validVariants -join ', ')"
        exit 2
    }
}

$fileName = Get-ModelFileName -ModelName $Model -VariantName $Variant
$modelUrl = "$HF_BASE/$fileName"
$modelDest = Join-Path $modelsDir $fileName

New-Item -ItemType Directory -Force -Path $modelsDir | Out-Null

Write-Host ""
Write-Host "  model:   $Model"
Write-Host "  variant: $Variant"
Write-Host "  file:    $fileName"
Write-Host "  dest:    $modelDest"
Write-Host ""

if (Test-Path $modelDest) {
    Write-Host "既に存在します: $modelDest"
} else {
    Invoke-Download -Url $modelUrl -Dest $modelDest -Label $fileName
}

# --- 完了メッセージ ---
Write-Host ""
Write-Host "セットアップ完了。"
Write-Host ""
Write-Host "次のステップ:"
Write-Host "  KoeType の設定画面で「文字起こしモード」から Whisper を選択してください。"
Write-Host ""
Write-Host "パスの確認:"
Write-Host "  whisper-cli: $cliDest"
Write-Host "  モデル:      $modelDest"
Write-Host ""
Write-Host "config.toml でパスを明示的に指定することもできます:"
Write-Host "  $appHome\config.toml"
Write-Host ""
Write-Host "  [win]"
Write-Host "  whisper_cli_path = `"$cliDest`""
