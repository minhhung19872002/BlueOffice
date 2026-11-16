param([string]$S,[string]$Which)
Add-Type -ReferencedAssemblies System.Drawing @"
using System; using System.Runtime.InteropServices; using System.Diagnostics; using System.Drawing; using System.Text;
public class Lat {
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
 [DllImport("user32.dll")] public static extern IntPtr FindWindowEx(IntPtr p, IntPtr a, string c, string t);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
 public delegate bool EnumProc(IntPtr h, IntPtr l);
 [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr p, EnumProc f, IntPtr l);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
 public static IntPtr FindChild(IntPtr p, string cls){ IntPtr r=IntPtr.Zero; EnumChildWindows(p,(h,l)=>{ var s=new StringBuilder(64); GetClassName(h,s,64); if(s.ToString()==cls){r=h;return false;} return true;},IntPtr.Zero); return r; }
 static long Sum(Bitmap b){ long s=0; for(int y=0;y<b.Height;y+=2) for(int x=0;x<b.Width;x+=2) s+=b.GetPixel(x,y).R*(x+1+y*7); return s; }
 public static double[] Run(IntPtr target, int x, int y, int w, int h, int n){
   var res=new double[n]; var bmp=new Bitmap(w,h); var g=Graphics.FromImage(bmp);
   for(int i=0;i<n;i++){
     g.CopyFromScreen(x,y,0,0,bmp.Size); long base0=Sum(bmp);
     var sw=Stopwatch.StartNew(); char c=(char)('a'+(i%26));
     PostMessage(target,0x102,(IntPtr)c,(IntPtr)1);
     double t=-1; while(sw.ElapsedMilliseconds<500){ g.CopyFromScreen(x,y,0,0,bmp.Size); if(Sum(bmp)!=base0){ t=sw.Elapsed.TotalMilliseconds; break;} }
     res[i]=t; System.Threading.Thread.Sleep(120);
   }
   return res;
 }
}
"@
[Lat]::SetProcessDpiAwarenessContext([IntPtr]-4)|Out-Null
if($Which -eq 'blue'){
  $env:BLUEOFFICE_ZOOM="100"; Copy-Item "D:\Source\BlueOffice\tests\corpus\doc\phase1-sample.docx" "$S\lat-b.docx" -Force
  $p=Start-Process "D:\Source\BlueOffice\target\release\bluedoc-native.exe" "$S\lat-b.docx" -PassThru; Start-Sleep 5
  & "$S\cap2.ps1" "lat-b.docx" "$S\tmp.png" | Out-Null; Start-Sleep 1
  $hw=$p.MainWindowHandle; [Lat]::SetForegroundWindow($hw)|Out-Null; Start-Sleep -m 300
  $l=[IntPtr]((612 -shl 16) -bor 600); [Lat]::PostMessage($hw,0x201,[IntPtr]1,$l)|Out-Null; [Lat]::PostMessage($hw,0x202,[IntPtr]0,$l)|Out-Null; Start-Sleep 1
  $r=[Lat]::Run($hw, 40+300, 40+300, 900, 500, 30); Stop-Process $p -Force
} else {
  Copy-Item "D:\Source\BlueOffice\tests\corpus\doc\phase1-sample.docx" "$S\lat-w.docx" -Force
  powershell.exe -NoProfile -File "$S\wopen.ps1" "$S\lat-w.docx" 100 | Out-Null
  & "$S\cap2.ps1" "lat-w.docx" "$S\tmp.png" | Out-Null
  $w=[Runtime.InteropServices.Marshal]::GetActiveObject("Word.Application"); $d=$null; foreach($x in $w.Documents){ if($x.Name -eq "lat-w.docx"){$d=$x} }
  $d.ReadOnlyRecommended=$false; $d.Paragraphs(9).Range.Select(); $w.Selection.Collapse(1)
  $hw=[IntPtr]$d.ActiveWindow.Hwnd; $doc=[Lat]::FindChild($hw,"_WwG"); [Lat]::SetForegroundWindow($hw)|Out-Null; Start-Sleep -m 500
  $r=[Lat]::Run($doc, 40+300, 40+300, 900, 500, 30); $d.Close(0)
}
($r | ForEach-Object { [math]::Round($_,1) }) -join ','
