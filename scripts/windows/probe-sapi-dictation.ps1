param(
  [int]$Seconds = 12,
  [string]$Culture = "ja-JP"
)

try {
  [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
  $OutputEncoding = [System.Text.UTF8Encoding]::new($false)
} catch {
  # ignore
}

Write-Host "SAPI PoC Start: Culture=$Culture, Duration=${Seconds}s"

try {
  Add-Type -AssemblyName System.Speech | Out-Null
} catch {
  Write-Host "Failed to load System.Speech. Use Windows PowerShell."
  Write-Host "Example: powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts\\probe-sapi-dictation.ps1"
  throw
}

function Show-InstalledRecognizers {
  try {
    $list = [System.Speech.Recognition.SpeechRecognitionEngine]::InstalledRecognizers()
    Write-Host "Installed Recognizers:"
    foreach ($r in $list) {
      $info = $r.RecognizerInfo
      $lang = $info.Culture.Name
      $name = $info.Name
      Write-Host ("- {0} ({1})" -f $name, $lang)
    }
  } catch {
    Write-Host "Failed to list recognizers."
  }
}

try {
  $rec = New-Object System.Speech.Recognition.SpeechRecognitionEngine($Culture)
} catch {
  Write-Host "Failed to create SpeechRecognitionEngine. Language pack may be missing."
  Show-InstalledRecognizers
  throw
}

if ($null -eq $rec) {
  Write-Host "SpeechRecognitionEngine is null."
  Show-InstalledRecognizers
  throw "SpeechRecognitionEngine was not initialized."
}

$grammar = New-Object System.Speech.Recognition.DictationGrammar
$rec.LoadGrammar($grammar) | Out-Null
$rec.SetInputToDefaultAudioDevice()

$rec.add_SpeechHypothesized({
  param($sender, $event)
  Write-Host ("[Hyp] " + $event.Result.Text)
})

$rec.add_SpeechRecognized({
  param($sender, $event)
  Write-Host ("[Final] " + $event.Result.Text)
})

$rec.add_SpeechRecognitionRejected({
  param($sender, $event)
  Write-Host "[Reject] Not recognized"
})

Write-Host "Start speaking. Duration: ${Seconds}s"
$rec.RecognizeAsync([System.Speech.Recognition.RecognizeMode]::Multiple)
Start-Sleep -Seconds $Seconds
$rec.RecognizeAsyncStop()

Write-Host "SAPI PoC End"
