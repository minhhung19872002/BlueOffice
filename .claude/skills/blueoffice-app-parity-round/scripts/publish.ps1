# publish.ps1 -Apps bluedoc-native,bluesheet-native [-From E:\bo-targets\main\release] [-Smoke]
# Copies freshly built native exes into out\desktop without killing anything the owner runs:
# a running exe is renamed to <name>.old.exe first. -Smoke launches each exe (no activation)
# on a copy of a sample file and captures it with tools\ui-compare\cap2.ps1 into $env:TEMP.
param(
  [string[]]$Apps,
  [string]$From = "E:\bo-targets\main\release",
  [switch]$Smoke
)
$repo = Resolve-Path "$PSScriptRoot\..\..\..\.."
$out = Join-Path $repo "out\desktop"
$samples = @{
  "bluedoc-native"    = "tests\corpus\doc\phase1-sample.docx"
  "bluesheet-native"  = "tests\corpus\sheet"   # first .xlsx in the folder
  "blueslide-native"  = "tests\corpus\slide"   # first .pptx in the folder
  "bluereader-native" = "crates\bo-sign\tests\data\ocsp\signed-hai.pdf"
  "blueeditor-native" = "crates\bo-sign\tests\data\ocsp\signed-hai.pdf"
}
foreach ($n in $Apps) {
  $exe = Join-Path $From "$n.exe"
  if (-not (Test-Path $exe)) { "missing $exe"; continue }
  if ($Smoke) {
    $src = Join-Path $repo $samples[$n]
    if ((Get-Item $src).PSIsContainer) {
      $ext = @{ "bluesheet-native" = "*.xlsx"; "blueslide-native" = "*.pptx" }[$n]; if (-not $ext) { $ext = "*.pdf" }
      $src = (Get-ChildItem $src -Filter $ext -Recurse | Select-Object -First 1).FullName
    }
    $copy = Join-Path $env:TEMP ("smoke-" + $n + [IO.Path]::GetExtension($src))
    Copy-Item $src $copy -Force
    $env:BLUEOFFICE_SHOW_NOACTIVATE = "1"
    $p = Start-Process $exe -ArgumentList "`"$copy`"" -PassThru; Start-Sleep 5
    try {
      & (Join-Path $repo "tools\ui-compare\cap2.ps1") ([IO.Path]::GetFileName($copy)) (Join-Path $env:TEMP "smoke-$n.png")
    } catch {
      "smoke capture failed for ${n}: $_ (not published)"
      $p | Stop-Process -ErrorAction SilentlyContinue
      continue
    }
    $p | Stop-Process -ErrorAction SilentlyContinue
  }
  $running = Get-Process $n -ErrorAction SilentlyContinue
  Push-Location $out
  Remove-Item "$n.next.exe" -ErrorAction SilentlyContinue
  if ($running) {
    Remove-Item "$n.old.exe" -ErrorAction SilentlyContinue
    if (Test-Path "$n.old.exe") { "skip $n (an older copy is still running)"; Pop-Location; continue }
    Rename-Item "$n.exe" "$n.old.exe"
  } else {
    Remove-Item "$n.old.exe", "$n.exe" -ErrorAction SilentlyContinue
  }
  Copy-Item $exe "$n.exe"
  Pop-Location
  "$n published (was running: $([bool]$running))"
}
