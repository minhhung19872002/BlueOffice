param([string]$S,[string]$Exe,[string]$Name,[string]$Zoom="100",[int]$W=1500,[int]$H=1102)
Add-Type @"
using System; using System.Runtime.InteropServices;
public class PM8 { [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v); }
"@
# Per-monitor aware like the app: a DPI-unaware sender has its posted mouse coordinates
# scaled by the system (209 px arrived as 261 at 125 %), so clicks hit the next control.
[PM8]::SetProcessDpiAwarenessContext([IntPtr]-4) | Out-Null
$log="$S\perf-$Name.jsonl"; Remove-Item $log -EA SilentlyContinue
$env:BLUEOFFICE_NATIVE_PERF=$log; $env:BLUEOFFICE_ZOOM=$Zoom
Copy-Item "D:\Source\BlueOffice\tests\corpus\doc\phase1-sample.docx" "$S\ty-$Name.docx" -Force
$p=Start-Process $Exe "$S\ty-$Name.docx" -PassThru; Start-Sleep 5
& "$S\cap2.ps1" "ty-$Name.docx" "$S\tmp.png" | Out-Null; Start-Sleep 1
$hw=$p.MainWindowHandle; $l=[IntPtr]((612 -shl 16) -bor 600)
[PM8]::PostMessage($hw,0x201,[IntPtr]1,$l)|Out-Null; [PM8]::PostMessage($hw,0x202,[IntPtr]0,$l)|Out-Null; Start-Sleep 1
$txt="nhanh hon nua di ban oi, go thu de do do tre cua ban phim nhe "
foreach($c in $txt.ToCharArray()){ [PM8]::PostMessage($hw,0x102,[IntPtr][int]$c,[IntPtr]1)|Out-Null; Start-Sleep -m 60 }
Start-Sleep 1; Stop-Process $p -Force; "ok"
