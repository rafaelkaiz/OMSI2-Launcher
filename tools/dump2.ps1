. "$PSScriptRoot\Win32.ps1"
$p = Get-Process Omsi | Select-Object -First 1
$lines = [W]::Dump($p.Id)
$on = $false
foreach ($l in $lines) {
  if ($l -notmatch '^\s') { $on = $l -match 'daytime' }
  if ($on) {
    $h = [IntPtr][Convert]::ToInt64(($l.Trim() -split ' ')[0], 16)
    "$l  @ " + [M]::Rect($h)
  }
}
