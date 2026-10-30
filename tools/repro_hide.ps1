param([string]$Out = "$env:TEMP")
# Dev helper: presses the "Ocultar el launcher" button of the loading screen (a mouse message sent to the window), then restores
# the window like the taskbar does and checks that it repaints.
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System; using System.Runtime.InteropServices;
public class RH {
  [StructLayout(LayoutKind.Sequential)] public struct R { public int L, T, Rt, B; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
}
"@
$env:OMSI2_OPEN = 'loading'
$p = Start-Process "$PSScriptRoot\..\target\release\omsi2-launcher.exe" -PassThru
Start-Sleep -Seconds 4; $p.Refresh(); $h = $p.MainWindowHandle
function Shot($name) {
  $r = New-Object RH+R; [RH]::GetWindowRect($h, [ref]$r) | Out-Null
  $w = $r.Rt - $r.L; $hh = $r.B - $r.T
  if ($w -le 0 -or $hh -le 0) { "${name}: no size"; return }
  $bmp = New-Object Drawing.Bitmap $w, $hh; $g = [Drawing.Graphics]::FromImage($bmp)
  $dc = $g.GetHdc(); [RH]::PrintWindow($h, $dc, 2) | Out-Null; $g.ReleaseHdc($dc)
  $bmp.Save("$Out\$name.png"); "${name}: ${w}x${hh}"
}
Shot 'hide_0'
$lp = [IntPtr](829 * 65536 + 960)
[RH]::PostMessage($h, 0x200, [IntPtr]0, $lp) | Out-Null   # mouse move over the button
Start-Sleep -Milliseconds 300
[RH]::PostMessage($h, 0x201, [IntPtr]1, $lp) | Out-Null   # button down
Start-Sleep -Milliseconds 80
[RH]::PostMessage($h, 0x202, [IntPtr]0, $lp) | Out-Null   # button up
Start-Sleep -Seconds 2
$p.Refresh()
"after the click: iconic=" + [RH]::IsIconic($h) + " responding=" + $p.Responding
[RH]::ShowWindow($h, 9) | Out-Null
Start-Sleep -Seconds 3
$p.Refresh()
"after restore: iconic=" + [RH]::IsIconic($h) + " responding=" + $p.Responding
Shot 'hide_1'
Start-Sleep -Seconds 2
Shot 'hide_2'
Stop-Process -Id $p.Id -Force
