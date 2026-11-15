param([string]$Title, [string]$Out, [int]$W=1500, [int]$H=1102)
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System; using System.Runtime.InteropServices; using System.Text;
public class U2 {
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h,IntPtr a,int x,int y,int cx,int cy,uint f);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
 public static IntPtr Find(string t){ IntPtr r=IntPtr.Zero; EnumWindows((h,l)=>{ var s=new StringBuilder(512); GetWindowText(h,s,512); if(IsWindowVisible(h) && s.ToString().Contains(t)){ r=h; return false;} return true;}, IntPtr.Zero); return r; }
}
"@
[U2]::SetProcessDpiAwarenessContext([IntPtr]-4) | Out-Null
$hw=[U2]::Find($Title); if($hw -eq [IntPtr]::Zero){ throw "no window $Title" }
[U2]::SetWindowPos($hw,[IntPtr]::Zero,40,40,$W,$H,0x14) | Out-Null
Start-Sleep -Milliseconds 1200
$bmp = New-Object System.Drawing.Bitmap -ArgumentList ([int]$W),([int]$H)
$g = [System.Drawing.Graphics]::FromImage($bmp); $dc=$g.GetHdc()
[U2]::PrintWindow($hw,$dc,2) | Out-Null; $g.ReleaseHdc($dc)
$bmp.Save($Out,[System.Drawing.Imaging.ImageFormat]::Png); "ok $Out"
