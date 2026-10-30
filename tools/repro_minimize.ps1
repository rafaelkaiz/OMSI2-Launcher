param([string]$Out = "$env:TEMP", [int]$Cycles = 3)
# Dev helper: shows the loading screen, minimises and restores it several times through Windows, and saves a picture of the window
# before and after each cycle, so a freeze after restoring shows as a clock that does not advance.
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System; using System.Runtime.InteropServices;
public class RM {
  [StructLayout(LayoutKind.Sequential)] public struct R { public int L, T, Rt, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
}
"@
$env:OMSI2_OPEN = 'loading'
$p = Start-Process "$PSScriptRoot\..\target\release\omsi2-launcher.exe" -PassThru
Start-Sleep -Seconds 4; $p.Refresh(); $h = $p.MainWindowHandle
function Shot($name) {
  $r = New-Object RM+R; [RM]::GetWindowRect($h, [ref]$r) | Out-Null
  $w = $r.Rt - $r.L; $hh = $r.B - $r.T
  if ($w -le 0 -or $hh -le 0) { "${name}: window has no size"; return }
  $bmp = New-Object Drawing.Bitmap $w, $hh; $g = [Drawing.Graphics]::FromImage($bmp)
  $dc = $g.GetHdc(); [RM]::PrintWindow($h, $dc, 2) | Out-Null; $g.ReleaseHdc($dc)
  $bmp.Save("$Out\$name.png"); "${name}: saved ${w}x${hh}"
}
Shot 'repro_0_start'
for ($i = 1; $i -le $Cycles; $i++) {
  [RM]::ShowWindow($h, 6) | Out-Null   # SW_MINIMIZE
  Start-Sleep -Seconds 2
  "cycle $i minimised: iconic=" + [RM]::IsIconic($h) + " responding=" + $p.Responding
  [RM]::ShowWindow($h, 9) | Out-Null   # SW_RESTORE
  Start-Sleep -Seconds 3
  $p.Refresh()
  "cycle $i restored: iconic=" + [RM]::IsIconic($h) + " responding=" + $p.Responding
  Shot "repro_${i}_after"
}
Stop-Process -Id $p.Id -Force
