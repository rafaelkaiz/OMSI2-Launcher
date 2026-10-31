param([string]$Omsi = 'C:\Program Files (x86)\Steam\steamapps\common\OMSI 2')
# Dev helper (read-only): for the options the launcher shows as sliders or number boxes, lists the smallest and largest value found in
# OMSI's own options.cfg and in every preset in option_presets\, to compare with the ranges used in options_layout.rs.
$keys = 'maxFPS','performance_tiledistmax','performance_maxObjDist','maxcomplexity','maxcomplexity_map','performance_reflTexSize','sound_vol_master','sound_stereo','sound_maxcount','AIMaxCountRandom','AIMaxCountScheduled','AIUnschedFactor','AIPriorityScheduled','AIMaxCountParked','AIPassFactor','smokesystems','screenratio','performance_dyn_redrefl','performance_dyn_tile_red','texmemlimit'
function Read-Cfg($p) {
  $b = [IO.File]::ReadAllBytes($p)
  if ($b.Length -ge 2 -and $b[0] -eq 0xFF -and $b[1] -eq 0xFE) { [Text.Encoding]::Unicode.GetString($b, 2, $b.Length - 2) } else { [Text.Encoding]::GetEncoding(1252).GetString($b) }
}
$files = @(Join-Path $Omsi 'options.cfg') + (Get-ChildItem (Join-Path $Omsi 'option_presets') -File -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName })
$found = @{}
foreach ($f in $files) {
  if (-not (Test-Path $f)) { continue }
  $lines = (Read-Cfg $f) -split "`r?`n"
  for ($i = 0; $i -lt $lines.Count; $i++) {
    $t = $lines[$i].Trim()
    if ($t -match '^\[(.+)\]$' -and $keys -contains $Matches[1]) {
      $t = $Matches[1]
      $vals = @(); $j = $i + 1
      while ($j -lt $lines.Count -and $lines[$j].Trim() -ne '' -and $lines[$j].Trim() -notmatch '^\[') { $vals += $lines[$j].Trim(); $j++ }
      for ($k = 0; $k -lt $vals.Count; $k++) {
        $d = 0.0
        if ([double]::TryParse($vals[$k], [Globalization.NumberStyles]::Float, [Globalization.CultureInfo]::InvariantCulture, [ref]$d)) {
          $id = "$t[$k]"
          if (-not $found.ContainsKey($id)) { $found[$id] = @{ min = $d; max = $d; n = 0 } }
          if ($d -lt $found[$id].min) { $found[$id].min = $d }
          if ($d -gt $found[$id].max) { $found[$id].max = $d }
          $found[$id].n++
        }
      }
    }
  }
}
"files read: $($files.Count)"
$found.GetEnumerator() | Sort-Object Name | ForEach-Object { "{0,-34} min={1,-10} max={2,-10} (n={3})" -f $_.Name, $_.Value.min, $_.Value.max, $_.Value.n }
