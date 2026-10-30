Add-Type @"
using System; using System.Text; using System.Collections.Generic; using System.Runtime.InteropServices;
public class W {
  public delegate bool EP(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EP cb, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr p, EP cb, IntPtr l);
  [DllImport("user32.dll")] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  public static List<string> Dump(uint pid) {
    var r = new List<string>();
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      if (p != pid) return true;
      r.Add(Desc(h, ""));
      EnumChildWindows(h, (c, l2) => { r.Add(Desc(c, "   ")); return true; }, IntPtr.Zero);
      return true;
    }, IntPtr.Zero);
    return r;
  }
  static string Desc(IntPtr h, string ind) {
    var c = new StringBuilder(256); var t = new StringBuilder(256);
    GetClassName(h, c, 256); GetWindowText(h, t, 256);
    return ind + h.ToInt64().ToString("X") + " [" + c + "] '" + t + "' vis=" + IsWindowVisible(h);
  }
}
"@
Add-Type @"
using System; using System.Text; using System.Runtime.InteropServices;
public class M {
  [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, StringBuilder l);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetParent(IntPtr h);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L,T,R,B; }
  public static string Rect(IntPtr h){ RECT r; GetWindowRect(h,out r); return r.L+","+r.T+" "+(r.R-r.L)+"x"+(r.B-r.T); }
  public static string[] Items(IntPtr h){
    int n=(int)SendMessage(h,0x146,IntPtr.Zero,IntPtr.Zero); var a=new string[n];
    for(int i=0;i<n;i++){ int len=(int)SendMessage(h,0x149,(IntPtr)i,IntPtr.Zero); var sb=new StringBuilder(len+2); SendMessage(h,0x148,(IntPtr)i,sb); a[i]=sb.ToString(); }
    return a; }
}
"@
