param([string]$S,[string]$Name,[string]$Clicks)
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System; using System.Runtime.InteropServices;
public class PM4 { [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);}
"@
[PM4]::SetProcessDpiAwarenessContext([IntPtr]-4)|Out-Null
$env:BLUEOFFICE_ZOOM="100"
$f="$S\$Name.docx"; Copy-Item "D:\Source\BlueOffice\tests\corpus\doc\phase1-sample.docx" $f -Force
$p=Start-Process "D:\Source\BlueOffice\target\release\bluedoc-native.exe" $f -PassThru; Start-Sleep 4
& "$S\cap2.ps1" "$Name.docx" "$S\tmp.png" | Out-Null
$hw=$p.MainWindowHandle; [PM4]::SetForegroundWindow($hw)|Out-Null; Start-Sleep -m 400
foreach($c in $Clicks.Split(';')){ if(-not $c){continue}; $xy=$c.Split(','); $l=[IntPtr](([int]$xy[1] -shl 16) -bor [int]$xy[0])
  [PM4]::PostMessage($hw,0x200,[IntPtr]0,$l)|Out-Null; [PM4]::PostMessage($hw,0x201,[IntPtr]1,$l)|Out-Null; [PM4]::PostMessage($hw,0x202,[IntPtr]0,$l)|Out-Null; Start-Sleep -m 900 }
$b=New-Object System.Drawing.Bitmap -ArgumentList 1500,700; $g=[System.Drawing.Graphics]::FromImage($b); $g.CopyFromScreen(40,40,0,0,$b.Size); $b.Save("$S\$Name.png")
Stop-Process $p -Force; "ok $Name"
