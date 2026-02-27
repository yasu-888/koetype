param(
  [int]$DurationSeconds = 30,
  [int]$PollMs = 200
)

try {
  [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
  $OutputEncoding = [System.Text.UTF8Encoding]::new($false)
} catch {
  # ignore
}

Write-Host "Win+H UIA PoC Start: Duration=${DurationSeconds}s, Poll=${PollMs}ms"

try {
  Add-Type -AssemblyName UIAutomationClient | Out-Null
  Add-Type -AssemblyName UIAutomationTypes | Out-Null
} catch {
  Write-Host "Failed to load UIAutomation. Use Windows PowerShell."
  throw
}

$windowNameHints = @("Voice typing", "Voice Typing", "Voice", "typing", "dictation")
$processHints = @("TextInputHost", "SearchApp", "ShellExperienceHost", "ApplicationFrameHost", "StartMenuExperienceHost")

function Find-VoiceTypingRoot {
  $root = [System.Windows.Automation.AutomationElement]::RootElement

  $types = @(
    [System.Windows.Automation.ControlType]::Window,
    [System.Windows.Automation.ControlType]::Pane
  )

  foreach ($type in $types) {
    $cond = New-Object System.Windows.Automation.PropertyCondition(
      [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
      $type
    )
    $items = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $cond)
    foreach ($w in $items) {
      $name = $w.Current.Name
      if ([string]::IsNullOrWhiteSpace($name)) { continue }
      foreach ($hint in $windowNameHints) {
        if ($name -like "*$hint*") { return $w }
      }
    }
  }

  $cond2 = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::IsOffscreenProperty, $false
  )
  $desc = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $cond2)
  foreach ($el in $desc) {
    $name = $el.Current.Name
    if ([string]::IsNullOrWhiteSpace($name)) { continue }
    foreach ($hint in $windowNameHints) {
      if ($name -like "*$hint*") { return $el }
    }
  }

  return $null
}

function Find-TextElement($root) {
  if ($null -eq $root) { return $null }

  $textCond = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::IsTextPatternAvailableProperty, $true
  )
  $desc = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $textCond)
  if ($desc.Count -gt 0) { return $desc.Item(0) }

  $valueCond = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::IsValuePatternAvailableProperty, $true
  )
  $desc2 = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $valueCond)
  if ($desc2.Count -gt 0) { return $desc2.Item(0) }

  return $null
}

function Dump-TopLevelWindows {
  $root = [System.Windows.Automation.AutomationElement]::RootElement
  $cond = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
    [System.Windows.Automation.ControlType]::Window
  )
  $windows = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $cond)
  Write-Host "Top windows:"
  $count = [Math]::Min(15, $windows.Count)
  for ($i = 0; $i -lt $count; $i++) {
    $w = $windows.Item($i)
    $name = $w.Current.Name
    if ([string]::IsNullOrWhiteSpace($name)) { $name = "(no name)" }
    Write-Host ("- " + $name)
  }
}

function Dump-DescendantHints {
  $root = [System.Windows.Automation.AutomationElement]::RootElement
  $cond = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::IsOffscreenProperty, $false
  )
  $desc = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $cond)
  Write-Host "Descendant hint candidates (visible only):"
  $shown = 0
  for ($i = 0; $i -lt $desc.Count -and $shown -lt 20; $i++) {
    $el = $desc.Item($i)
    $name = $el.Current.Name
    $autoId = $el.Current.AutomationId
    $className = $el.Current.ClassName
    $framework = $el.Current.FrameworkId
    $procId = $el.Current.ProcessId
    $procName = ""
    try { $procName = (Get-Process -Id $procId -ErrorAction SilentlyContinue).ProcessName } catch {}
    $text = "$name $autoId $className $framework"
    $match = $false
    foreach ($hint in $windowNameHints) {
      if ($text -match $hint) { $match = $true; break }
    }
    $procMatch = $false
    foreach ($ph in $processHints) {
      if ($procName -match $ph) { $procMatch = $true; break }
    }
    if ($match -or $procMatch) {
      Write-Host ("- name='{0}' autoId='{1}' class='{2}' fw='{3}' proc='{4}' pid={5}" -f $name, $autoId, $className, $framework, $procName, $procId)
      $shown++
    }
  }
}

function Dump-DescendantHintsAll {
  $root = [System.Windows.Automation.AutomationElement]::RootElement
  $condAll = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::IsOffscreenProperty, $true
  )
  $descAll = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $condAll)
  Write-Host "Descendant hint candidates (offscreen only):"
  $shown = 0
  for ($i = 0; $i -lt $descAll.Count -and $shown -lt 20; $i++) {
    $el = $descAll.Item($i)
    $name = $el.Current.Name
    $autoId = $el.Current.AutomationId
    $className = $el.Current.ClassName
    $framework = $el.Current.FrameworkId
    $procId = $el.Current.ProcessId
    $procName = ""
    try { $procName = (Get-Process -Id $procId -ErrorAction SilentlyContinue).ProcessName } catch {}
    $text = "$name $autoId $className $framework"
    $match = $false
    foreach ($hint in $windowNameHints) {
      if ($text -match $hint) { $match = $true; break }
    }
    $procMatch = $false
    foreach ($ph in $processHints) {
      if ($procName -match $ph) { $procMatch = $true; break }
    }
    if ($match -or $procMatch) {
      Write-Host ("- name='{0}' autoId='{1}' class='{2}' fw='{3}' proc='{4}' pid={5}" -f $name, $autoId, $className, $framework, $procName, $procId)
      $shown++
    }
  }
}

$start = Get-Date
$lastText = ""
$window = $null
$textEl = $null
$lastDump = Get-Date

Write-Host "Open Win+H voice typing panel."

while (((Get-Date) - $start).TotalSeconds -lt $DurationSeconds) {
  if ($null -eq $window) {
    $window = Find-VoiceTypingRoot
    if ($null -ne $window) {
      Write-Host "Voice typing panel detected."
      $textEl = Find-TextElement $window
      if ($null -eq $textEl) {
        Write-Host "Text element not found."
      }
    }
  }

  if ($null -eq $window -and ((Get-Date) - $lastDump).TotalSeconds -ge 3) {
    Dump-TopLevelWindows
    Dump-DescendantHints
    Dump-DescendantHintsAll
    $lastDump = Get-Date
  }

  if ($null -ne $textEl) {
    try {
      $text = ""
      $tmp = $null
      if ($textEl.TryGetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern, [ref]$tmp)) {
        $pattern = $textEl.GetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern)
        $range = $pattern.DocumentRange
        $text = $range.GetText(-1).Trim()
      } elseif ($textEl.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$tmp)) {
        $pattern2 = $textEl.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
        $text = $pattern2.Current.Value.Trim()
      }
      if ($text -ne $lastText -and $text -ne "") {
        Write-Host ("[UIA] " + $text)
        $lastText = $text
      }
    } catch {
      $window = $null
      $textEl = $null
      $lastText = ""
    }
  }

  Start-Sleep -Milliseconds $PollMs
}

Write-Host "Win+H UIA PoC End"
