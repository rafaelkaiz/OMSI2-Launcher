param([string]$Exe, [string[]]$Addrs)
# Read-only: finds where the code of a Delphi program stores the address of some methods (for example "mov [reg+off], offset Handler").
$b = [IO.File]::ReadAllBytes($Exe)
$latin = [Text.Encoding]::GetEncoding(28591)
$text = $latin.GetString($b)
$pe = [BitConverter]::ToInt32($b, 0x3c)
$nsec = [BitConverter]::ToUInt16($b, $pe + 6)
$opt = $pe + 24
$base = [BitConverter]::ToUInt32($b, $opt + 28)
$so = $opt + [BitConverter]::ToUInt16($b, $pe + 20)
$secs = for ($i = 0; $i -lt $nsec; $i++) {
  $s = $so + $i * 40
  [pscustomobject]@{ Va = [BitConverter]::ToUInt32($b, $s + 12); Raw = [BitConverter]::ToUInt32($b, $s + 20); RawSize = [BitConverter]::ToUInt32($b, $s + 16) }
}
function Off2Va([int]$o) { foreach ($s in $secs) { if ($o -ge $s.Raw -and $o -lt $s.Raw + $s.RawSize) { return [uint32]($o - $s.Raw + $s.Va + $base) } }; 0 }
foreach ($a in ($Addrs -join "," -split ",")) {
  $va = [Convert]::ToUInt32($a, 16)
  $pat = $latin.GetString([BitConverter]::GetBytes($va))
  $pos = 0
  while (($i = $text.IndexOf($pat, $pos, [StringComparison]::Ordinal)) -ge 0) {
    $pos = $i + 1
    $from = [Math]::Max(0, $i - 10)
    $hex = ($b[$from..($i + 9)] | ForEach-Object { '{0:X2}' -f $_ }) -join ' '
    "{0}  found at VA 0x{1:X8}: {2}" -f $a, (Off2Va $i), $hex
  }
}
