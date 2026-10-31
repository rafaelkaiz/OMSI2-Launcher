param(
  [string]$Omsi = 'C:\Program Files (x86)\Steam\steamapps\common\OMSI 2',
  [string]$Out = "$PSScriptRoot\..\docs\options.md"
)
# Dev helper (read-only on OMSI): writes docs\options.md from three sources: the launcher's own options window code
# (src\options_layout.rs: which control edits which key, inverted boxes, ranges), OMSI's English texts (Languages\ENG_*.olf) and
# what is really stored in options.cfg and in every option preset (how many files have each key, with which values).
$utf8 = New-Object Text.UTF8Encoding($false)
$cp = [Text.Encoding]::GetEncoding(1252)
$inv = [Globalization.CultureInfo]::InvariantCulture

function Read-Text($p) {
  $b = [IO.File]::ReadAllBytes($p)
  if ($b.Length -ge 2 -and $b[0] -eq 0xFF -and $b[1] -eq 0xFE) { [Text.Encoding]::Unicode.GetString($b, 2, $b.Length - 2) } else { $cp.GetString($b) }
}

# OMSI's English texts
$eng = @{}
foreach ($f in Get-ChildItem "$Omsi\Languages" -Filter 'ENG_*.olf') {
  foreach ($l in ((Read-Text $f.FullName) -split "`r?`n")) {
    $i = $l.IndexOf("`t")
    if ($i -gt 0) { $eng[$l.Substring(0, $i)] = $l.Substring($i + 1).Replace('@', ' ').Trim() }
  }
}

# What the files hold: for every key, in how many files and which values on each value line
function Parse-Cfg($p) {
  $r = [ordered]@{}
  $lines = (Read-Text $p) -split "`r?`n"
  for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i].Trim() -match '^\[([^\]]+)\]$') {
      $name = $Matches[1]; $vals = @(); $j = $i + 1
      while ($j -lt $lines.Count -and $lines[$j].Trim() -ne '' -and $lines[$j].Trim() -notmatch '^\[' -and $lines[$j] -notmatch '-{4,}') { $vals += $lines[$j].Trim(); $j++ }
      $r[$name] = $vals
    }
  }
  $r
}
$files = @("$Omsi\options.cfg") + @(Get-ChildItem "$Omsi\option_presets" -Filter *.oop | ForEach-Object FullName)
$stat = @{}
foreach ($f in $files) {
  foreach ($e in (Parse-Cfg $f).GetEnumerator()) {
    $k = $e.Key
    if (-not $stat.ContainsKey($k)) { $stat[$k] = @{ files = 0; width = 0; vals = @{} } }
    $stat[$k].files++
    $stat[$k].width = [Math]::Max($stat[$k].width, $e.Value.Count)
    for ($n = 0; $n -lt $e.Value.Count; $n++) { $stat[$k].vals["$n|$($e.Value[$n])"] = 1 + [int]$stat[$k].vals["$n|$($e.Value[$n])"] }
  }
}
$total = $files.Count

# The values seen on value line $n of a key
function Column($k, $n) {
  $vals = @($stat[$k].vals.Keys | Where-Object { $_ -like "$n|*" } | ForEach-Object { $_.Substring($_.IndexOf('|') + 1) } | Sort-Object)
  if (-not $vals.Count) { return '-' }
  $nums = @($vals | ForEach-Object { $d = 0.0; if ([double]::TryParse($_, [Globalization.NumberStyles]::Float, $inv, [ref]$d)) { $d } })
  if ($vals.Count -le 5) { return ($vals -join ' / ') }
  if ($nums.Count -eq $vals.Count) { $m = $nums | Measure-Object -Minimum -Maximum; return ('{0} to {1}' -f $m.Minimum.ToString($inv), $m.Maximum.ToString($inv)) }
  'free text'
}
function Describe($k, $idx) {
  if (-not $stat.ContainsKey($k)) { return 'not in any file' }
  if ($stat[$k].width -eq 0) { return 'on/off: present = on' }
  if ($null -ne $idx) { return (Column $k $idx) }
  (0..($stat[$k].width - 1) | ForEach-Object { Column $k $_ }) -join '; '
}
function Files($k) { if ($stat.ContainsKey($k)) { $stat[$k].files } else { 0 } }

# The launcher's options window: which control edits which key
$src = Get-Content "$PSScriptRoot\..\src\options_layout.rs" -Encoding UTF8
$tabOf = @{ general = 'General'; advanced = 'Advanced'; graphics = 'Graphics'; graphics_advanced = 'Graphics (advanced)'; sound = 'Sound'; traffic = 'AI traffic' }
$rows = [ordered]@{}
foreach ($t in 'General', 'Advanced', 'Graphics', 'Graphics (advanced)', 'Sound', 'AI traffic') { $rows[$t] = New-Object System.Collections.Generic.List[object] }
function Add-Row($tab, $text, $key, $how, $idx) {
  $rows[$tab].Add([pscustomobject]@{ Text = $text; Key = $key; How = $how; Seen = $(if ($how -eq 'text box') { 'free text' } else { Describe $key $idx }); Files = (Files $key) })
}
$tab = $null; $label = ''
foreach ($line in $src) {
  if ($line -match '^fn (general|advanced|graphics|graphics_advanced|sound|traffic)\(cv') { $tab = $tabOf[$Matches[1]]; $label = ''; continue }
  if (-not $tab) { continue }
  $id = $null
  if ($line -match 'cv\.t\("(DG_Options_[A-Za-z0-9_]+)"') { $id = $Matches[1] }
  if ($line -match 'cv\.(label|note)\(') { if ($id) { $label = $id }; continue }
  # the material channels are built from a list of (text, key)
  if ($tab -eq 'Graphics (advanced)' -and $line -match '^\s*\("([^"]+)", "(no_[A-Za-z_]+)"\),') {
    Add-Row $tab $Matches[1] $Matches[2] 'check box, **inverted**: the key is present when the box is OFF' $null; continue
  }
  # the master volume has its own slider code
  if ($tab -eq 'Sound' -and $line -match 'Slider::new\(&mut vol') {
    Add-Row $tab $eng['DG_Options_Label10'] 'sound_vol_master' 'slider with 10 steps (1 to 10): the file holds the step divided by 10 (0.1 to 1.0)' $null; continue
  }
  if ($line -match 'Slider::new\(&mut size') {
    Add-Row $tab $eng['DG_Options_Label28'] 'performance_reflTexSize' 'slider 0 to 12: the texture is 2 to the power of the value (up to 4096 pixels)' $null; continue
  }
  if ($line -notmatch 'cv\.(flag|int_bool|on_off|number|slider_i|slider_f|combo|text_box)\(|percent\(cv') { continue }
  $name = if ($id) { $id } else { $label }
  $key = $null
  if ($line -match '"([A-Za-z0-9_]+)",\s*"(GENERAL|GRAPHICS|MULTITHREADING|SOUND|AI)"') { $key = $Matches[1] }
  elseif ($line -match 'percent\(cv, ui, o, [\d.]+, [\d.]+, "([A-Za-z0-9_]+)"') { $key = $Matches[1] }
  if (-not $key) { continue }
  $idx = $null
  if ($line -match '"(?:GENERAL|GRAPHICS|MULTITHREADING|SOUND|AI)",\s*(\d+)\b') { $idx = [int]$Matches[1] }
  elseif ($line -match 'cv\.int_bool\(.*?,\s*(\d+)\)\s*;') { $idx = [int]$Matches[1] }
  $how = ''
  if ($line -match 'cv\.flag\(') { $how = if ($line -match '"[A-Z_]+", true\)') { 'check box, **inverted**: the key is present when the box is OFF' } else { 'check box: present = ticked' } }
  elseif ($line -match 'cv\.on_off\(') { $how = 'check box stored as `on` / `off`' }
  elseif ($line -match 'cv\.int_bool\(') { $how = "check box stored as 0 / 1 on value line $($idx + 1)" }
  elseif ($line -match 'cv\.number\(') { $how = $(if ($null -ne $idx -and $idx -gt 0) { "number box, value line $($idx + 1)" } else { 'number box' }) }
  elseif ($line -match 'slider_i\(.*?\((-?\d+), (-?\d+)\)') { $how = "slider $($Matches[1]) to $($Matches[2])" + $(if ($idx) { ", value line $($idx + 1)" } else { '' }) }
  elseif ($line -match 'slider_f\(.*?\(([\d.]+), ([\d.]+)\)') { $how = "slider $($Matches[1]) to $($Matches[2])" }
  elseif ($line -match 'percent\(cv.*?, ([\d.]+), (true|false),') { $how = "slider 0 to $($Matches[1]) % of the screen size (the file holds a fraction); " + $(if ($Matches[2] -eq 'true') { 'runs backwards: the biggest value is at the left end' } else { 'normal direction' }) }
  elseif ($line -match 'cv\.combo\(') { $how = 'drop-down list' }
  elseif ($line -match 'cv\.text_box\(') { $how = 'text box' }
  $text = if ($eng.ContainsKey($name)) { $eng[$name] } else { $name }
  if (-not $text) { $text = 'Manual screen ratio' }
  Add-Row $tab $text $key $how $idx
}
# the one check box that sets both multithreading keys together
foreach ($k in 'no_multithreading_calculate', 'no_multithreading_texload') {
  Add-Row 'Advanced' $eng['DG_Options_CheckBox61'] $k 'one check box that sets both keys together; **inverted**: present when OFF' $null
}

$handled = @{}; foreach ($t in $rows.Keys) { foreach ($r in $rows[$t]) { $handled[$r.Key] = 1 } }
$game = [ordered]@{
  'last_map' = 'last map chosen on the start screen (`maps\<map>\global.cfg`)'
  'last_driver' = 'last driver chosen (`Drivers\<name>.odr`)'
  'currWeather_ICAO' = 'weather station of the start screen'
  'nopreview' = 'hide the bus previews when choosing a vehicle inside the game'
  'gamectrleron' = 'game controller switched on; OMSI sets it itself when it finds a wheel or pad'
  'no_tex_low_high_switch' = 'switching between low and high resolution textures'
  'font_typewriter' = 'font used for the typewriter texts'
  'texFilter' = 'texture filter: two values'
  'texture' = 'texture setting: two values (0 and 1 in every preset of OMSI)'
  'Editor_LinkAerial' = 'address used by the map editor to get aerial pictures (contains a key of the map service; do not share it)'
}

$sb = New-Object Text.StringBuilder
function W($s) { [void]$sb.AppendLine($s) }
W '# OMSI 2 options reference'
W ''
W 'What each key of the `options.cfg` of OMSI 2 and of the option presets (`option_presets\*.oop`) means, which control of the launcher edits it and what a preset does with it. The tables of keys are generated by `tools\gen_options_doc.ps1` from the launcher code, the English texts of OMSI and the files of a real installation, so they can be regenerated after a change.'
W ''
W "Counts are over **$total files**: the `options.cfg` of the installation and the option presets that OMSI 2 and its add-ons install. A preset has the same format as `options.cfg`."
W ''
W '## File format'
W ''
W '- A key is a line `[name]`; the lines under it, until a blank line, are its values.'
W '- A key with **no value lines** is an on/off switch: present means on, absent means off. Some are inverted (`no_...`): present means the feature is off. The "How" column says which.'
W '- Comment lines such as ` GENERAL ------------------------ ` only group the keys; OMSI writes the keys in its own order.'
W '- The files are plain Windows-1252 text with CR LF line ends. The launcher keeps every line it does not edit exactly as it was.'
W ''
W '## What loading a preset does (checked against OMSI 2 itself)'
W ''
W 'The launcher loads a preset as OMSI does. This was checked by loading test presets in the game and reading `options.cfg` afterwards.'
W ''
W '| Case | What happens |'
W '| --- | --- |'
W '| A key the preset has, edited by the Options window (including language, last map and last driver) | It is copied, and the start screen follows the new map and driver. |'
W '| An on/off box of the Options window that the preset does not have | It is switched **off**. |'
W '| `radio` or `screenratio` (a radio link, a manual screen ratio) that the preset does not have | It is **removed**: they only exist while in use. |'
W '| `texture` | Copied from the preset. |'
W '| `nopreview`, `texFilter`, `font_typewriter`, `Editor_LinkAerial`, `no_tex_low_high_switch`, `gamectrleron` | **Not touched** by a preset, in either direction. |'
W '| Any other value the preset does not have (older presets do not know newer options) | Kept as it was. |'
W ''
W 'Nothing is written to `options.cfg` until the user saves (Options, OK, or Start); a backup is made first, and the 3 newest are kept.'
W ''
foreach ($t in $rows.Keys) {
  W "## Tab: $t"
  W ''
  W '| Setting (text of OMSI) | Key | How | Values seen | In files |'
  W '| --- | --- | --- | --- | --- |'
  foreach ($r in $rows[$t]) { $c1 = ([string]$r.Text).Replace('|', '/'); $c2 = ([string]$r.Seen).Replace('|', '/'); W "| $c1 | ``$($r.Key)`` | $($r.How) | $c2 | $($r.Files)/$total |" }
  W ''
}
W '## Keys the game saves itself'
W ''
W 'No tab of the Options window edits these; OMSI writes them while you play or from its other windows.'
W ''
W '| Key | Meaning | Values seen | In files |'
W '| --- | --- | --- | --- |'
foreach ($k in $game.Keys) { $seen = 'an address'; if ($k -ne 'Editor_LinkAerial') { $seen = (Describe $k $null).Replace('|', '/') }; W "| ``$k`` | $($game[$k]) | $seen | $(Files $k)/$total |" }
W ''
$other = @($stat.Keys | Where-Object { -not $handled.ContainsKey($_) -and -not $game.Contains($_) } | Sort-Object)
if ($other.Count) {
  W '## Other keys found'
  W ''
  foreach ($k in $other) { W "- ``$k`` ($(Files $k)/$total files; $(Describe $k $null))" }
  W ''
}
W '## Not covered yet'
W ''
W 'The launcher does not edit the **Keyboard**, **Game Controller** and **Addons** tabs of the Options window of OMSI (use the game''s own window for those).'
[IO.File]::WriteAllText($Out, $sb.ToString(), $utf8)
"written: $Out ($(($rows.Values | ForEach-Object { $_.Count } | Measure-Object -Sum).Sum) rows, other keys: $($other.Count))"
