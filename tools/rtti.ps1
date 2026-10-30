param([string]$Exe, [string]$Class = 'Tform_start')
# Read-only: lists the published methods (name + address) of a Delphi class, found through its VMT.
$b = [IO.File]::ReadAllBytes($Exe)
$pe = [BitConverter]::ToInt32($b, 0x3c)
$nsec = [BitConverter]::ToUInt16($b, $pe + 6)
$opt = $pe + 24
$base = [BitConverter]::ToUInt32($b, $opt + 28)
$so = $opt + [BitConverter]::ToUInt16($b, $pe + 20)
$secs = for ($i = 0; $i -lt $nsec; $i++) {
  $s = $so + $i * 40
  [pscustomobject]@{ Va = [BitConverter]::ToUInt32($b, $s + 12); Size = [BitConverter]::ToUInt32($b, $s + 8); Raw = [BitConverter]::ToUInt32($b, $s + 20); RawSize = [BitConverter]::ToUInt32($b, $s + 16) }
}
function Va2Off([uint32]$va) { $r = [uint32]($va - $base); foreach ($s in $secs) { if ($r -ge $s.Va -and $r -lt $s.Va + [Math]::Min($s.Size, $s.RawSize)) { return [int]($r - $s.Va + $s.Raw) } }; -1 }
function Off2Va([int]$o) { foreach ($s in $secs) { if ($o -ge $s.Raw -and $o -lt $s.Raw + $s.RawSize) { return [uint32]($o - $s.Raw + $s.Va + $base) } }; 0 }
function U32([int]$o) { [BitConverter]::ToUInt32($b, $o) }
function PStr([int]$o) { [Text.Encoding]::GetEncoding(28591).GetString($b, $o + 1, $b[$o]) }

$text = [Text.Encoding]::GetEncoding(28591).GetString($b)
$needle = [string][char]$Class.Length + $Class
$pos = 0
$found = $false
while (($i = $text.IndexOf($needle, $pos, [StringComparison]::Ordinal)) -ge 0) {
  $pos = $i + 1
  $va = Off2Va $i
  if ($va -eq 0) { continue }
  $pat = [BitConverter]::GetBytes($va)
  # every place that stores a pointer to this class-name string
  $j = 0
  while (($j = $text.IndexOf([Text.Encoding]::GetEncoding(28591).GetString($pat), $j, [StringComparison]::Ordinal)) -ge 0) {
    $slot = $j; $j++
    foreach ($layout in @(@{ name = 'XE2+'; self = -88; cls = -56; meth = -64; parent = -48 }, @{ name = 'D7-XE'; self = -76; cls = -44; meth = -52; parent = -36 })) {
      $vmt = (Off2Va $slot) - $layout.cls
      if ($vmt -le 0) { continue }
      $selfOff = Va2Off ([uint32]($vmt + $layout.self))
      if ($selfOff -lt 0 -or (U32 $selfOff) -ne [uint32]$vmt) { continue }
      $found = $true
      "class {0}: VMT at 0x{1:X8} (layout {2}), instance size {3}" -f $Class, $vmt, $layout.name, (U32 (Va2Off ([uint32]($vmt - ($layout.cls + 0) + ($layout.cls + 4)))))
      $parent = U32 (Va2Off ([uint32]($vmt + $layout.parent)))
      "  parent VMT pointer slot value: 0x{0:X8}" -f $parent
      $mt = U32 (Va2Off ([uint32]($vmt + $layout.meth)))
      if ($mt -eq 0) { '  no published methods'; continue }
      $o = Va2Off $mt
      $count = [BitConverter]::ToUInt16($b, $o); $o += 2
      "  published methods: $count"
      for ($m = 0; $m -lt $count; $m++) {
        $len = [BitConverter]::ToUInt16($b, $o)
        $addr = U32 ($o + 2)
        "    0x{0:X8}  {1}" -f $addr, (PStr ($o + 6))
        $o += $len
      }
    }
  }
}
if (-not $found) { "class $Class not found" }
