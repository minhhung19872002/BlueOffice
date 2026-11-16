# drive.ps1 <S> <name> <clicks "x,y;x,y;..."> : launch fresh doc, click sequence (client px), capture
param([string]$S,[string]$Name,[string]$Clicks)
Add-Type @"
using System; using System.Runtime.InteropServices;
public class PM3 { [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l); }
"@
$env:BLUEOFFICE_ZOOM="100"
$f="$S\$Name.docx"; Copy-Item "D:\Source\BlueOffice\tests\corpus\doc\phase1-sample.docx" $f -Force
$p=Start-Process "D:\Source\BlueOffice\target\release\bluedoc-native.exe" $f -PassThru; Start-Sleep 4
& "$S\cap2.ps1" "$Name.docx" "$S\tmp.png" | Out-Null
$hw=$p.MainWindowHandle
foreach($c in $Clicks.Split(';')){ if(-not $c){continue}; $xy=$c.Split(','); $l=[IntPtr](([int]$xy[1] -shl 16) -bor [int]$xy[0])
  [PM3]::PostMessage($hw,0x200,[IntPtr]0,$l)|Out-Null; [PM3]::PostMessage($hw,0x201,[IntPtr]1,$l)|Out-Null; [PM3]::PostMessage($hw,0x202,[IntPtr]0,$l)|Out-Null; Start-Sleep -m 900 }
& "$S\cap2.ps1" "$Name.docx" "$S\$Name.png" | Out-Null
Stop-Process $p -Force; "ok $Name"
