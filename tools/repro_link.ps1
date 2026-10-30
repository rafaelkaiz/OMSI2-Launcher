# Dev helper: opens the launcher the normal way (full screen), clicks the forum link with mouse messages sent to the window,
# and reports whether the launcher is still alive afterwards. Opens the web browser on the forum page.
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System; using System.Runtime.InteropServices;
public class RL {
  [StructLayout(LayoutKind.Sequential)] public struct R { public int L, T, Rt, B; }
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out R r);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
}
"@
$p = Start-Process "$PSScriptRoot\..\target\release\omsi2-launcher.exe" -PassThru
Start-Sleep -Seconds 8; $p.Refresh(); $h = $p.MainWindowHandle
$r = New-Object RL+R; [RL]::GetClientRect($h, [ref]$r) | Out-Null
"client ${($r.Rt)}x$($r.B)"
$x = [int](($r.Rt - 1080) / 2 + 267); $y = [int](($r.B - 780) / 2 + 112)
$lp = [IntPtr]($y * 65536 + $x)
[RL]::PostMessage($h, 0x200, [IntPtr]0, $lp) | Out-Null; Start-Sleep -Milliseconds 300
[RL]::PostMessage($h, 0x201, [IntPtr]1, $lp) | Out-Null; Start-Sleep -Milliseconds 80
[RL]::PostMessage($h, 0x202, [IntPtr]0, $lp) | Out-Null
Start-Sleep -Seconds 4
$p.Refresh(); "alive after click: " + (-not $p.HasExited)
if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force }
