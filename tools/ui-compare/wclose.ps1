$w=[Runtime.InteropServices.Marshal]::GetActiveObject("Word.Application"); foreach($d in @($w.Documents)){ if($d.Name -eq "wd-try.docx"){ $d.Close(0); "closed" } }
