param([string]$F,[int]$Z)
$w=[Runtime.InteropServices.Marshal]::GetActiveObject("Word.Application")
$d=$null; foreach($x in $w.Documents){ if($x.FullName -eq $F){$d=$x} }
if(-not $d){ $d=$w.Documents.Open($F,$false,$true) }
$win=$d.ActiveWindow; $win.View.Type=3; $win.View.Zoom.Percentage=$Z; $win.WindowState=0
$d.Range(0,0).Select() | Out-Null; $win.ScrollIntoView($d.Range(0,0))
$d.ShowSpellingErrors=$false; $d.ShowGrammaticalErrors=$false
"zoom " + $win.View.Zoom.Percentage
