param([string]$Path)
$b = [IO.File]::ReadAllBytes($Path)
$pe = [BitConverter]::ToInt32($b, 0x3c)
$mach = [BitConverter]::ToUInt16($b, $pe + 4)
$magic = [BitConverter]::ToUInt16($b, $pe + 24)
$opt = $pe + 24
$dd = if ($magic -eq 0x20b) { $opt + 112 } else { $opt + 96 }
$rva = [BitConverter]::ToUInt32($b, $dd)
$nsec = [BitConverter]::ToUInt16($b, $pe + 6)
$so = $opt + [BitConverter]::ToUInt16($b, $pe + 20)
function R2O([uint32]$r) {
  for ($i = 0; $i -lt $nsec; $i++) {
    $s = $so + $i * 40
    $va = [BitConverter]::ToUInt32($b, $s + 12); $vs = [BitConverter]::ToUInt32($b, $s + 8); $ro = [BitConverter]::ToUInt32($b, $s + 20)
    if ($r -ge $va -and $r -lt $va + $vs) { return [int]($r - $va + $ro) }
  }
  return -1
}
"{0}: machine=0x{1:X}" -f (Split-Path $Path -Leaf), $mach
if ($rva -eq 0) { '  no export table'; exit }
$eo = R2O $rva
$n = [BitConverter]::ToUInt32($b, $eo + 24)
$np = R2O ([BitConverter]::ToUInt32($b, $eo + 32))
$fp = R2O ([BitConverter]::ToUInt32($b, $eo + 28))
$op = R2O ([BitConverter]::ToUInt32($b, $eo + 36))
for ($i = 0; $i -lt $n; $i++) {
  $o = R2O ([BitConverter]::ToUInt32($b, $np + 4 * $i)); $e = $o
  while ($b[$e] -ne 0) { $e++ }
  $name = [Text.Encoding]::ASCII.GetString($b, $o, $e - $o)
  $ord = [BitConverter]::ToUInt16($b, $op + 2 * $i)
  $fo = R2O ([BitConverter]::ToUInt32($b, $fp + 4 * $ord))
  # follow a leading "jmp rel32" (incremental-link thunk), then list the `ret imm16` (C2 xx xx) bytes in the next 400 bytes
  if ($b[$fo] -eq 0xE9) { $fo = $fo + 5 + [BitConverter]::ToInt32($b, $fo + 1) }
  $rets = @()
  for ($k = 0; $k -lt 400 -and ($fo + $k + 2) -lt $b.Length; $k++) {
    if ($b[$fo + $k] -eq 0xC2) { $rets += [BitConverter]::ToUInt16($b, $fo + $k + 1) }
  }
  "{0,-24} ret N candidates: {1}" -f $name, (($rets | Select-Object -Unique | Select-Object -First 4) -join ', ')
}
'--- first bytes of PluginStart / PluginFinalize (after following one jmp thunk)'
for ($i = 0; $i -lt $n; $i++) {
  $o = R2O ([BitConverter]::ToUInt32($b, $np + 4 * $i)); $e = $o
  while ($b[$e] -ne 0) { $e++ }
  $name = [Text.Encoding]::ASCII.GetString($b, $o, $e - $o)
  if ($name -notin 'PluginStart', 'PluginFinalize') { continue }
  $ord = [BitConverter]::ToUInt16($b, $op + 2 * $i)
  $fo = R2O ([BitConverter]::ToUInt32($b, $fp + 4 * $ord))
  if ($b[$fo] -eq 0xE9) { $fo = $fo + 5 + [BitConverter]::ToInt32($b, $fo + 1) }
  $hex = ($b[$fo..($fo + 39)] | ForEach-Object { '{0:X2}' -f $_ }) -join ' '
  "{0,-15} {1}" -f $name, $hex
}
'--- epilogues (5D C2 nn nn = pop ebp; ret nn  /  5D C3 = pop ebp; ret) within 6000 bytes'
for ($i = 0; $i -lt $n; $i++) {
  $o = R2O ([BitConverter]::ToUInt32($b, $np + 4 * $i)); $e = $o
  while ($b[$e] -ne 0) { $e++ }
  $name = [Text.Encoding]::ASCII.GetString($b, $o, $e - $o)
  if ($name -notin 'PluginStart', 'PluginFinalize') { continue }
  $ord = [BitConverter]::ToUInt16($b, $op + 2 * $i)
  $fo = R2O ([BitConverter]::ToUInt32($b, $fp + 4 * $ord))
  if ($b[$fo] -eq 0xE9) { $fo = $fo + 5 + [BitConverter]::ToInt32($b, $fo + 1) }
  $found = @()
  for ($k = 0; $k -lt 6000 -and ($fo + $k + 3) -lt $b.Length; $k++) {
    if ($b[$fo + $k] -eq 0x5D -and $b[$fo + $k + 1] -eq 0xC2) { $found += "ret " + [BitConverter]::ToUInt16($b, $fo + $k + 2) }
    elseif ($b[$fo + $k] -eq 0x5D -and $b[$fo + $k + 1] -eq 0xC3) { $found += 'ret' }
  }
  "{0,-15} {1}" -f $name, (($found | Select-Object -First 3) -join ' | ')
}
