# physical-idle.ps1 — owner-idle guard for real keyboard / mouse input (owner decision 2026-11-24).
#
# Agents may send real input (SendInput / keybd_event / SetCursorPos / agent-desktop --headed) to the
# reference app ONLY while the owner is physically idle. Windows' GetLastInputInfo cannot tell the
# owner's input from an agent's synthetic input, so this watcher installs low-level keyboard and
# mouse hooks and records only events WITHOUT the injected flag (LLKHF_INJECTED / LLMHF_INJECTED).
#
#   powershell -File physical-idle.ps1 -Start        # start the watcher (no-op when one runs)
#   powershell -File physical-idle.ps1 -Check 10     # exit 0 + "idle N s" when no physical input for
#                                                    # >= 10 min, else exit 1 + "busy N s"
#   powershell -File physical-idle.ps1 -Stop
#
# State file: %TEMP%\bo-physical-input.txt (UTC ticks of the last physical input, written at most 1/s).
param([switch]$Start, [double]$Check = -1, [switch]$Stop, [switch]$Run)

$state = Join-Path $env:TEMP 'bo-physical-input.txt'
$pidFile = Join-Path $env:TEMP 'bo-physical-input.pid'

function Get-Watcher {
  if (Test-Path $pidFile) {
    $id = [int](Get-Content $pidFile -ErrorAction SilentlyContinue)
    $p = Get-Process -Id $id -ErrorAction SilentlyContinue
    if ($p) { return $p }
  }
  $null
}

if ($Start) {
  if (Get-Watcher) { 'watcher running'; exit 0 }
  # Until the hook sees anything, count the owner as active "now" (never assume idle at start).
  [DateTime]::UtcNow.Ticks | Set-Content $state
  $p = Start-Process powershell -WindowStyle Hidden -PassThru `
    -ArgumentList '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$PSCommandPath`"", '-Run'
  $p.Id | Set-Content $pidFile
  "watcher started (pid $($p.Id))"
  exit 0
}

if ($Stop) {
  $p = Get-Watcher
  if ($p) { $p | Stop-Process -Force; 'watcher stopped' } else { 'no watcher' }
  Remove-Item $pidFile -ErrorAction SilentlyContinue
  exit 0
}

if ($Check -ge 0) {
  if (-not (Get-Watcher)) { 'busy: watcher not running (start it with -Start, then wait)'; exit 1 }
  $last = [long](Get-Content $state -ErrorAction SilentlyContinue)
  $secs = [int](([DateTime]::UtcNow.Ticks - $last) / 1e7)
  if ($secs -ge $Check * 60) { "idle $secs s"; exit 0 } else { "busy $secs s"; exit 1 }
}

if ($Run) {
  Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Windows.Forms;
public static class PhysicalInput {
  delegate IntPtr Proc(int code, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] static extern IntPtr SetWindowsHookEx(int id, Proc fn, IntPtr mod, uint tid);
  [DllImport("user32.dll")] static extern IntPtr CallNextHookEx(IntPtr h, int code, IntPtr w, IntPtr l);
  [DllImport("kernel32.dll")] static extern IntPtr GetModuleHandle(string name);
  static Proc kb = KeyHook, ms = MouseHook;
  static string path; static long lastWrite;
  static void Mark() {
    long now = DateTime.UtcNow.Ticks;
    if (now - lastWrite < 10000000) return;       // at most once a second
    lastWrite = now;
    try { File.WriteAllText(path, now.ToString()); } catch { }
  }
  // KBDLLHOOKSTRUCT.flags at offset 8, bit 0x10 = LLKHF_INJECTED.
  static IntPtr KeyHook(int code, IntPtr w, IntPtr l) {
    if (code >= 0 && (Marshal.ReadInt32(l, 8) & 0x10) == 0) Mark();
    return CallNextHookEx(IntPtr.Zero, code, w, l);
  }
  // MSLLHOOKSTRUCT.flags at offset 12, bit 0x01 = LLMHF_INJECTED.
  static IntPtr MouseHook(int code, IntPtr w, IntPtr l) {
    if (code >= 0 && (Marshal.ReadInt32(l, 12) & 0x01) == 0) Mark();
    return CallNextHookEx(IntPtr.Zero, code, w, l);
  }
  public static void Run(string statePath) {
    path = statePath;
    IntPtr mod = GetModuleHandle(null);
    SetWindowsHookEx(13, kb, mod, 0);   // WH_KEYBOARD_LL
    SetWindowsHookEx(14, ms, mod, 0);   // WH_MOUSE_LL
    Application.Run();                  // message loop the LL hooks need
  }
}
'@ -ReferencedAssemblies System.Windows.Forms
  [PhysicalInput]::Run($state)
}
