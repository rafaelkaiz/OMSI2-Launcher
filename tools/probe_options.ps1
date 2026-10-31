# Dev helper (read-only): lists every window of the running Omsi.exe with its class, title, position and, for list boxes and
# combo boxes, the items. Used to learn what an OMSI dialog looks like (e.g. the "where do you want to appear" window).
Add-Type @"
using System; using System.Text; using System.Collections.Generic; using System.Runtime.InteropServices;
public class PW {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [StructLayout(LayoutKind.Sequential)] public struct R { public int L, T, Rt, B; }
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr h, EnumProc p, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, StringBuilder l);
  [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  public static List<IntPtr> Tops(uint pid) {
    var r = new List<IntPtr>();
    EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p); if (p == pid) r.Add(h); return true; }, IntPtr.Zero);
    return r;
  }
  public static List<IntPtr> Kids(IntPtr parent) {
    var r = new List<IntPtr>();
    EnumChildWindows(parent, (h, l) => { r.Add(h); return true; }, IntPtr.Zero);
    return r;
  }
  public static string Cls(IntPtr h) { var s = new StringBuilder(256); GetClassName(h, s, 256); return s.ToString(); }
  public static string Txt(IntPtr h) { var s = new StringBuilder(512); GetWindowText(h, s, 512); return s.ToString(); }
  public static string Rect(IntPtr h) { R r; GetWindowRect(h, out r); return r.L + "," + r.T + " " + (r.Rt - r.L) + "x" + (r.B - r.T); }
  public static List<string> Items(IntPtr h, bool combo) {
    var r = new List<string>();
    uint count = combo ? 0x146u : 0x18Bu, text = combo ? 0x148u : 0x189u, len = combo ? 0x149u : 0x18Au;
    int n = (int)SendMessage(h, count, IntPtr.Zero, IntPtr.Zero);
    for (int i = 0; i < n && i < 500; i++) {
      var s = new StringBuilder(512); SendMessage(h, text, (IntPtr)i, s); r.Add(s.ToString());
    }
    return r;
  }
}
"@
$omsi = Get-Process Omsi -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $omsi) { "Omsi.exe is not running"; return }
Add-Type @"
using System; using System.Runtime.InteropServices;
public class TB { [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l); }
"@
# Dev helper (read-only): the options form of the running OMSI: every slider with its real range and position, every combo box with items.
foreach ($t in [PW]::Tops([uint32]$omsi.Id)) {
  if ([PW]::Cls($t) -ne 'Tform_options') { continue }
  "FORM Tform_options rect=" + [PW]::Rect($t)
  foreach ($k in [PW]::Kids($t)) {
    $c = [PW]::Cls($k)
    if ($c -match 'trackbar') {
      $min = [int][TB]::SendMessage($k, 0x401, [IntPtr]0, [IntPtr]0); $max = [int][TB]::SendMessage($k, 0x402, [IntPtr]0, [IntPtr]0); $pos = [int][TB]::SendMessage($k, 0x400, [IntPtr]0, [IntPtr]0)
      "SLIDER {0} rect={1} min={2} max={3} pos={4}" -f $k, [PW]::Rect($k), $min, $max, $pos
    } elseif ($c -match 'ComboBox') {
      "COMBO {0} rect={1} items=[{2}]" -f $k, [PW]::Rect($k), (([PW]::Items($k, $true)) -join ' | ')
    } elseif ($c -match 'Edit|Memo') {
      "EDIT {0} rect={1} text='{2}'" -f $k, [PW]::Rect($k), [PW]::Txt($k)
    } elseif ($c -match 'Button|Check|Radio|Group|Label|Static') {
      "{0} {1} rect={2} text='{3}'" -f $c.ToUpper(), $k, [PW]::Rect($k), [PW]::Txt($k)
    }
  }
}
