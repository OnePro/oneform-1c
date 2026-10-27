Add-Type -AssemblyName System.Windows.Forms,System.Drawing
Add-Type @"
using System; using System.Runtime.InteropServices;
public class W { [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h,int c); [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h); [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h,int x,int y,int w,int ht,bool r);}
"@
(New-Object -ComObject Shell.Application).MinimizeAll()
Start-Sleep -Seconds 1
foreach($p in Get-Process 1cv8 -ErrorAction SilentlyContinue){ if($p.MainWindowHandle -ne 0){ [W]::ShowWindow($p.MainWindowHandle,3) | Out-Null; [W]::SetForegroundWindow($p.MainWindowHandle) | Out-Null } }
Start-Sleep -Seconds 2
$b=[System.Windows.Forms.SystemInformation]::VirtualScreen
$bmp=New-Object System.Drawing.Bitmap $b.Width,$b.Height
$g=[System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($b.Left,$b.Top,0,0,$bmp.Size)
$bmp.Save($args[0])
