# M3 Step 6 -- verify supervision from OUTSIDE the agent's own process tree.
#
# Spike 001 could not measure orphaning: the host runtime anchors its descendants
# with a Windows Job Object, so "the harness would orphan" and "the harness killed
# it" were indistinguishable. A Scheduled Task runs under the Task Scheduler
# service, outside that job. A survivor here is a real survivor.
#
# It also sets the working directory to C:\Windows\System32 -- the exact
# condition spike 002 produced for the packaged app. If path resolution were
# cwd-dependent, the sidecar would never start at all.

param(
    [string]$Exe = "$PSScriptRoot\..\src-tauri\target\debug\capybaras-shell.exe",
    [int]$WaitSeconds = 30
)

$ErrorActionPreference = 'Continue'
$taskName = 'Capybaras-Supervision-Verify'
$stateDir = Join-Path $env:LOCALAPPDATA 'Capybaras'
$log = Join-Path $stateDir 'sidecar.log'
$sys32 = 'C:\Windows\System32'

function Get-Sidecars {
    Get-CimInstance Win32_Process -Filter "Name='node.exe'" -ErrorAction SilentlyContinue |
        Where-Object { $_.CommandLine -like '*sidecar.mjs*' }
}

Write-Host '=== pre-clean ==='
Get-Sidecars | ForEach-Object {
    Write-Host "  killing leftover sidecar pid=$($_.ProcessId)"
    Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue
}
Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue | Unregister-ScheduledTask -Confirm:$false
Remove-Item $log -Force -ErrorAction SilentlyContinue
Write-Host '  state log cleared, previous task removed'

if (-not (Test-Path $Exe)) { Write-Host "FATAL: no binary at $Exe"; exit 1 }

Write-Host ''
Write-Host '=== launch OUTSIDE this process tree, cwd = C:\Windows\System32 ==='
$action = New-ScheduledTaskAction -Execute $Exe -WorkingDirectory $sys32
$principal = New-ScheduledTaskPrincipal -UserId "$env:USERDOMAIN\$env:USERNAME" -LogonType Interactive -RunLevel Limited
Register-ScheduledTask -TaskName $taskName -Action $action -Principal $principal -Force | Out-Null
Start-ScheduledTask -TaskName $taskName

Write-Host '  waiting for shell + sidecar...'
$shell = $null
$sidecar = $null
$deadline = (Get-Date).AddSeconds($WaitSeconds)
while ((Get-Date) -lt $deadline) {
    if (-not $shell) {
        $shell = Get-Process -Name 'capybaras-shell' -ErrorAction SilentlyContinue | Select-Object -First 1
    }
    $sidecar = Get-Sidecars | Select-Object -First 1
    if ($shell -and $sidecar) { break }
    Start-Sleep -Milliseconds 400
}

Write-Host ''
if ($shell) { Write-Host "  shell   : pid=$($shell.Id)" } else { Write-Host '  shell   : NOT RUNNING' }
if ($sidecar) { Write-Host "  sidecar : pid=$($sidecar.ProcessId)" } else { Write-Host '  sidecar : NOT RUNNING' }

Write-Host ''
Write-Host '=== did path resolution survive cwd = System32? ==='
if (Test-Path $log) {
    $text = Get-Content $log -Raw
    Write-Host "  log found at the plain, non-virtualised path:"
    ($text -split "`n" | Select-Object -First 8) | ForEach-Object { Write-Host ('    ' + $_.TrimEnd()) }
    if ($text -match 'sidecar up') {
        Write-Host '  PATH RESOLUTION: OK -- sidecar started with cwd = system32'
    }
    else {
        Write-Host '  PATH RESOLUTION: sidecar never reported startup'
    }
}
else {
    Write-Host '  no sidecar.log -- the sidecar never started'
}

if (-not $shell) {
    Write-Host ''
    Write-Host 'cannot continue: no shell process to kill'
    Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue | Unregister-ScheduledTask -Confirm:$false
    exit 1
}

Write-Host ''
Write-Host '=== HARD KILL the shell (no cleanup, no handlers, no chance to be polite) ==='
taskkill /PID $shell.Id /F 2>&1 | ForEach-Object { Write-Host ('  ' + $_) }
Start-Sleep -Seconds 5

Write-Host ''
Write-Host '=== ORPHAN CHECK ==='
$after = @(Get-Sidecars)
if ($after.Count -gt 0) {
    Write-Host "  RESULT: ORPHANED -- $($after.Count) sidecar process(es) survived the shell"
    $after | ForEach-Object { Write-Host "    pid=$($_.ProcessId)" }
    $after | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
}
else {
    Write-Host '  RESULT: NO ORPHANS -- the sidecar died with the shell'
}

Write-Host ''
Write-Host '=== cleanup ==='
Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue | Unregister-ScheduledTask -Confirm:$false
Get-Process -Name 'capybaras-shell' -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Get-Sidecars | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
Write-Host '  task unregistered, processes cleared'
