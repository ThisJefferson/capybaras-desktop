# SPIKE HARNESS — throwaway.
#
# Three scenarios against the same sidecar:
#   1. clean       — supervisor shuts the sidecar down normally
#   2. crash       — supervisor is force-killed; does the sidecar orphan?
#   3. crash-watch — same, but the sidecar watches its parent
#
# The question the spike exists to answer: on Windows, does killing a supervisor
# leave its Node sidecar running? And does a parent-watch fix it?

$ErrorActionPreference = 'Continue'
$here = "C:\Users\Skept\.openclaw\workspace\projects\safe-agent-desktop\spikes\001-sidecar-supervision"
$node = (Get-Command node).Source

function Test-Alive([int]$ProcessId) {
  if (-not $ProcessId) { return $false }
  return [bool](Get-Process -Id $ProcessId -ErrorAction SilentlyContinue)
}

function Stop-Quietly([int]$ProcessId) {
  if ($ProcessId) { Stop-Process -Id $ProcessId -Force -ErrorAction SilentlyContinue }
}

$results = @()

function Invoke-Scenario([string]$Mode, [string]$Label, [switch]$CrashSupervisor) {
  $pidFile = Join-Path $here ".sidecar-$Mode.pid"
  $outFile = Join-Path $here ".out-$Mode.txt"
  $errFile = Join-Path $here ".err-$Mode.txt"
  Remove-Item $pidFile, $outFile, $errFile -Force -ErrorAction SilentlyContinue

  $sup = Start-Process -FilePath $node `
    -ArgumentList @((Join-Path $here 'supervisor.mjs'), $Mode, $pidFile) `
    -RedirectStandardOutput $outFile -RedirectStandardError $errFile `
    -PassThru -WindowStyle Hidden

  Start-Sleep -Seconds 2

  $sidecarPid = 0
  if (Test-Path $pidFile) {
    $raw = (Get-Content $pidFile -Raw).Trim()
    if ($raw -match '^\d+$') { $sidecarPid = [int]$raw }
  }

  $supAliveBefore = Test-Alive $sup.Id
  $sidecarAliveBefore = Test-Alive $sidecarPid

  if ($CrashSupervisor) {
    # Simulate a hard crash of the supervisor: no cleanup, no chance to run exit handlers.
    Stop-Quietly $sup.Id
  }

  Start-Sleep -Seconds 4

  $supAliveAfter = Test-Alive $sup.Id
  $sidecarAliveAfter = Test-Alive $sidecarPid

  $verdict = if (-not $sidecarAliveBefore) { 'sidecar never started' }
             elseif ($sidecarAliveAfter) { 'ORPHANED' }
             else { 'cleaned up' }

  $script:results += [pscustomobject]@{
    Scenario        = $Label
    SidecarPid      = $sidecarPid
    SupBefore       = $supAliveBefore
    StartingAlive   = $sidecarAliveBefore
    SupAfter        = $supAliveAfter
    SidecarAfter    = $sidecarAliveAfter
    Verdict         = $verdict
  }

  # Tidy up whatever survived.
  Stop-Quietly $sidecarPid
  Stop-Quietly $sup.Id
  Start-Sleep -Milliseconds 300
}

"=== running scenarios ==="
Invoke-Scenario -Mode 'clean'       -Label '1. clean shutdown'
Invoke-Scenario -Mode 'crash'       -Label '2. supervisor crash'            -CrashSupervisor
Invoke-Scenario -Mode 'crash-watch' -Label '3. supervisor crash + parent-watch' -CrashSupervisor

""
"=== RESULTS ==="
$results | Format-Table -AutoSize | Out-String -Width 200

""
"=== sweep for leftovers ==="
$leftover = Get-Process -Name node -ErrorAction SilentlyContinue | Where-Object { $_.Id -ne $PID }
"  stray node processes: " + ($leftover | Measure-Object).Count
$leftover | ForEach-Object { "    pid=" + $_.Id }
