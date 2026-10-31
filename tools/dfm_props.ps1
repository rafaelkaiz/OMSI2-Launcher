param([string]$Exe, [string]$Class = 'Tform_start')
# Read-only: prints the components of a Delphi form (binary DFM resource) with position and event handlers.
$b = [IO.File]::ReadAllBytes($Exe)
$latin = [Text.Encoding]::GetEncoding(28591)
$text = $latin.GetString($b)
$needle = 'TPF0' + [string][char]$Class.Length + $Class
$start = $text.IndexOf($needle, [StringComparison]::Ordinal)
if ($start -lt 0) { "form resource for $Class not found"; exit }
$script:p = $start + 4

function ReadPStr {
  $n = $b[$script:p]
  $s = $latin.GetString($b, $script:p + 1, $n)
  $script:p += 1 + $n
  return $s
}

function ReadValue {
  $t = $b[$script:p]; $script:p++
  if ($t -eq 0) { return $null }
  if ($t -eq 1) { $items = @(); while ($b[$script:p] -ne 0) { $items += (ReadValue) }; $script:p++; return '[' + ($items -join ' | ') + ']' }
  if ($t -eq 2) { $v = [int]$b[$script:p]; if ($v -gt 127) { $v -= 256 }; $script:p += 1; return $v }
  if ($t -eq 3) { $v = [BitConverter]::ToInt16($b, $script:p); $script:p += 2; return $v }
  if ($t -eq 4) { $v = [BitConverter]::ToInt32($b, $script:p); $script:p += 4; return $v }
  if ($t -eq 5) { $script:p += 10; return '<ext>' }
  if ($t -eq 6) { return (ReadPStr) }
  if ($t -eq 7) { return '@' + (ReadPStr) }
  if ($t -eq 8) { return $false }
  if ($t -eq 9) { return $true }
  if ($t -eq 10) { $n = [BitConverter]::ToInt32($b, $script:p); $script:p += 4 + $n; return '<binary>' }
  if ($t -eq 11) { while ($b[$script:p] -ne 0) { $null = ReadPStr }; $script:p++; return '[set]' }
  if ($t -eq 12) { $n = [BitConverter]::ToInt32($b, $script:p); $s = $latin.GetString($b, $script:p + 4, $n); $script:p += 4 + $n; return $s }
  if ($t -eq 13) { return $null }
  if ($t -eq 14) {
    while ($b[$script:p] -ne 0) {
      if ($b[$script:p] -eq 1 -or $b[$script:p] -eq 2 -or $b[$script:p] -eq 3 -or $b[$script:p] -eq 4) { $null = ReadValue }
      while ($b[$script:p] -ne 0) { $null = ReadPStr; $null = ReadValue }
      $script:p++
    }
    $script:p++
    return '<collection>'
  }
  if ($t -eq 15) { $script:p += 4; return '<single>' }
  if ($t -eq 16) { $script:p += 8; return '<currency>' }
  if ($t -eq 17) { $script:p += 8; return '<date>' }
  if ($t -eq 18) { $n = [BitConverter]::ToInt32($b, $script:p); $script:p += 4 + 2 * $n; return '<wstring>' }
  if ($t -eq 19) { $script:p += 8; return '<int64>' }
  if ($t -eq 20) { $n = [BitConverter]::ToInt32($b, $script:p); $s = [Text.Encoding]::UTF8.GetString($b, $script:p + 4, $n); $script:p += 4 + $n; return $s }
  throw "unknown value type $t at $($script:p)"
}

function ReadObject([int]$depth) {
  if (($b[$script:p] -band 0xF0) -eq 0xF0) { $flags = $b[$script:p]; $script:p++; if ($flags -band 2) { $null = ReadValue } }
  $cls = ReadPStr
  $name = ReadPStr
  $props = [ordered]@{}
  while ($b[$script:p] -ne 0) { $k = ReadPStr; $props[$k] = ReadValue }
  $script:p++
  $ev = ($props.Keys | Where-Object { $_ -like 'On*' } | ForEach-Object { "$_=$($props[$_])" }) -join ' '
  $extra = (@("Min","Max","Position","Increment","Value","MinValue","MaxValue","Text","Caption","Items","ItemIndex","Checked","Tag","Frequency","PageSize") | Where-Object { $props.Contains($_) } | ForEach-Object { "$_=$($props[$_])" }) -join "  "
  "{0}{1}: {2}  L={3} T={4} W={5} H={6}  {7}  {8}" -f ("  " * $depth), $name, $cls, $props["Left"], $props["Top"], $props["Width"], $props["Height"], $ev, $extra
  while ($b[$script:p] -ne 0) { ReadObject ($depth + 1) }
  $script:p++
}

ReadObject 0
