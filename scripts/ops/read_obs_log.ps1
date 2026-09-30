$latestLog = Get-ChildItem "$env:APPDATA\obs-studio\logs" -Filter *.txt | Sort-Object LastWriteTime -Descending | Select-Object -First 1
Write-Host "Reading log: $($latestLog.FullName)"
Get-Content $latestLog.FullName | Select-String -Pattern "BlewRed", "blewred" -Context 0, 2
