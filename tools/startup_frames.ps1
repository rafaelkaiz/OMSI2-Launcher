param([string]$Out = "$env:TEMP\sf", [int]$Count = 30)
# Dev helper: starts the launcher and captures the whole screen every ~80 ms, to see what the window does while it opens.
Add-Type -AssemblyName System.Drawing, System.Windows.Forms
New-Item -ItemType Directory -Force $Out | Out-Null
Remove-Item "$Out\*.png" -ErrorAction SilentlyContinue
$b = [Windows.Forms.SystemInformation]::VirtualScreen
$p = Start-Process "$PSScriptRoot\..\target\release\omsi2-launcher.exe" -PassThru
for ($i = 0; $i -lt $Count; $i++) {
  $bmp = New-Object Drawing.Bitmap $b.Width, $b.Height
  $g = [Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($b.X, $b.Y, 0, 0, $bmp.Size)
  $small = New-Object Drawing.Bitmap $bmp, 480, 270
  $small.Save("$Out\f{0:D2}.png" -f $i); $bmp.Dispose(); $small.Dispose(); $g.Dispose()
  Start-Sleep -Milliseconds 60
}
Stop-Process -Id $p.Id -Force
