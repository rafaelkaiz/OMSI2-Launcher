param([int]$Index)
. "$PSScriptRoot\Win32.ps1"
$combo=[IntPtr]0x709B4; $form=[IntPtr]0x70968; $memo=[IntPtr]0xB099C
function Txt($h){ $sb=New-Object Text.StringBuilder 300; [M]::SendMessage($h,0xD,[IntPtr]300,$sb)|Out-Null; $sb.ToString() -replace "`r`n",' ' }
"antes : " + (Txt $memo).Substring(0,70)
[M]::SendMessage($combo,0x14E,[IntPtr]$Index,[IntPtr]::Zero)|Out-Null
$id=[M]::GetDlgCtrlID($combo) -band 0xFFFF
[M]::SendMessage($form,0x111,[IntPtr](($id) -bor (1 -shl 16)),$combo)|Out-Null
Start-Sleep -Milliseconds 800
"despues: " + (Txt $memo).Substring(0,70)
