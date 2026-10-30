param([string]$Out, [string]$Open = 'options', [int]$Tab = 0)
# Dev helper: starts the launcher with a window open and saves a picture of that window only (PrintWindow: nothing else on screen is captured).
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System; using System.Runtime.InteropServices;
public class SH {
  [StructLayout(LayoutKind.Sequential)] public struct R { public int L, T, Rt, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
}
"@
$env:OMSI2_OPEN = $Open
$env:OMSI2_TAB = "$Tab"
$p = Start-Process "$PSScriptRoot\..\target\release\omsi2-launcher.exe" -PassThru
$r = New-Object SH+R
$h = [IntPtr]::Zero
for ($i = 0; $i -lt 60; $i++) {
  Start-Sleep -Milliseconds 250; $p.Refresh(); $h = $p.MainWindowHandle
  if ($h -ne [IntPtr]::Zero) { [SH]::GetWindowRect($h, [ref]$r) | Out-Null; if (($r.Rt - $r.L) -gt 400) { break } }
}
Start-Sleep -Seconds 2
[SH]::GetWindowRect($h, [ref]$r) | Out-Null
$w = $r.Rt - $r.L; $hh = $r.B - $r.T
$bmp = New-Object Drawing.Bitmap $w, $hh
$g = [Drawing.Graphics]::FromImage($bmp)
$dc = $g.GetHdc()
[SH]::PrintWindow($h, $dc, 2) | Out-Null
$g.ReleaseHdc($dc)
$bmp.Save($Out)
Stop-Process -Id $p.Id -Force
"saved $w x $hh"
