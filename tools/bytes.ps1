param([string]$Exe, [string]$Va, [int]$Count = 64)
# Read-only: hex dump of the bytes at a virtual address of the program.
$b = [IO.File]::ReadAllBytes($Exe)
$pe = [BitConverter]::ToInt32($b, 0x3c)
$nsec = [BitConverter]::ToUInt16($b, $pe + 6)
$opt = $pe + 24
$base = [BitConverter]::ToUInt32($b, $opt + 28)
$so = $opt + [BitConverter]::ToUInt16($b, $pe + 20)
$v = [Convert]::ToUInt32($Va, 16)
$r = [uint32]($v - $base)
for ($i = 0; $i -lt $nsec; $i++) {
  $s = $so + $i * 40
  $sva = [BitConverter]::ToUInt32($b, $s + 12); $raw = [BitConverter]::ToUInt32($b, $s + 20); $rs = [BitConverter]::ToUInt32($b, $s + 16)
  if ($r -ge $sva -and $r -lt $sva + $rs) {
    $o = [int]($r - $sva + $raw)
    for ($k = 0; $k -lt $Count; $k += 16) {
      $line = ($b[($o + $k)..([Math]::Min($o + $k + 15, $o + $Count - 1))] | ForEach-Object { '{0:X2}' -f $_ }) -join ' '
      "0x{0:X8}: {1}" -f ($v + $k), $line
    }
    exit
  }
}
'address not in a section'
