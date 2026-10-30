. "$PSScriptRoot\Win32.ps1"
# Waits for an Omsi.exe started by the user and logs every change in its visible windows.
$last = ''
for ($i = 0; $i -lt 600; $i++) {
  $p = Get-Process Omsi -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($p) {
    $v = ([W]::Dump($p.Id) | Where-Object { $_ -match 'vis=True' }) -join "`n"
    if ($v -ne $last) { "[$i s] pid $($p.Id)`n$v`n"; $last = $v }
  } elseif ($last) { "exited"; break }
  Start-Sleep 1
}
