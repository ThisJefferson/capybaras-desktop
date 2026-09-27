# Renders the interface for visual review.
#
# WHY THIS SCRIPT EXISTS: doing these steps by hand went wrong. The harness is a
# COPY of index.html with a mocked Tauri bridge injected, and it was generated
# once and then reused after index.html had changed -- so a screenshot "proved"
# the absence of a button that was actually there. A stale instrument is worse
# than no instrument, because it produces confident wrong answers.
#
# Generating the harness and screenshotting are therefore ONE step, in one
# script, so they cannot drift apart.
#
# Usage:  powershell -NoProfile -ExecutionPolicy Bypass -File verify-ui.ps1
#         [-Port 8791] [-Width 940] [-Height 1060]

param(
    [int]$Port = 8791,
    [int]$Width = 940,
    [int]$Height = 1060
)

$ErrorActionPreference = 'Stop'

$here = $PSScriptRoot
# NOTE: Join-Path in Windows PowerShell takes only TWO positional arguments
# (-Path and -ChildPath). Passing a third throws "A positional parameter cannot
# be found", the script dies on this line, and the harness silently is NOT
# regenerated -- which is exactly the stale-instrument failure this script
# exists to prevent. Keep it to one path string.
$web = [System.IO.Path]::GetFullPath((Join-Path $here '..\web'))
$index = Join-Path $web 'index.html'
$harness = Join-Path $web 'verify-card.html'
$shot = Join-Path $web 'card-verify.png'
$server = Join-Path $env:USERPROFILE '.openclaw\workspace\.openclaw\tmp\serve-static.mjs'
$chrome = 'C:\Program Files\Google\Chrome\Application\chrome.exe'

if (-not (Test-Path $chrome)) { throw "Chrome not found at $chrome" }
if (-not (Test-Path $server)) { throw "Static server not found at $server" }

# ---------------------------------------------------------------------------
# 1. Regenerate the harness from the CURRENT index.html, every time.
# ---------------------------------------------------------------------------
Write-Host '=== regenerating the harness from index.html ==='
$html = [System.IO.File]::ReadAllText($index, [System.Text.Encoding]::UTF8)

$mock = @'
<script>
// VERIFICATION HARNESS -- not shipped. Mocks the Tauri bridge so the REAL
// app.js and app.css render for review.
window.__TAURI__ = {
  invoke: async function (cmd) {
    if (cmd === 'shell_status') {
      return { shell_pid: 4242, state_dir: 'C:\\Users\\Skept\\AppData\\Local\\Capybaras',
               sidecar_running: true, sidecar_pid: 5150, job_assigned: true };
    }
    return null;
  },
  event: {
    listen: function (name, cb) {
      if (name === 'capybaras://agents') {
        setTimeout(function () {
          cb({ payload: { agents: [
            { id:'tuca', label:'Tuca', job:'the one you talk to',   accent:'atlantic', state:'listening', sinceMs: 900 },
            { id:'bia',  label:'Bia',  job:'looks things up',        accent:'sky',      state:'working',   sinceMs: 220 },
            { id:'zeca', label:'Zeca', job:'checks the work',        accent:'ink',      state:'listening', sinceMs: 4000 },
            { id:'nina', label:'Nina', job:'writes and edits files', accent:'leaf',     state:'needs-you', sinceMs: 120 },
            { id:'joca', label:'Joca', job:'runs commands',          accent:'capy',     state:'dozing',    sinceMs: 8000 },
            { id:'duda', label:'Duda', job:'tidies and organises',   accent:'sand',     state:'listening', sinceMs: 700 }
          ] } });
        }, 100);
      }
      if (name === 'capybaras://approval-required') {
        setTimeout(function () {
          cb({ payload: { id: 'demo-1', request: {
            tier: 'hard_gate', headline: 'Delete 1,200 records?',
            reasons: [
              'This target is marked off-limits.',
              'This would affect 1200 things at once.',
              'This cannot be undone.'
            ],
            confirmationPhrase: 'Delete 1,200 records', canRemember: false,
            requiresTypedConfirmation: true, target: 'the production database' } } });
        }, 200);
      }
    }
  }
};
</script>
'@

$anchor = '<script type="module" src="app.js">'
if (-not $html.Contains($anchor)) { throw "could not find the app.js script tag in index.html" }
[System.IO.File]::WriteAllText($harness, $html.Replace($anchor, $mock + $anchor), (New-Object System.Text.UTF8Encoding($false)))
Write-Host "  harness written: $harness"

# ---------------------------------------------------------------------------
# 2. Serve it over HTTP. file:// will NOT work: Chrome blocks ES modules there,
#    so app.js never runs and the screenshot is meaningless.
# ---------------------------------------------------------------------------
Write-Host '=== serving over http ==='
$proc = Start-Process -FilePath 'node' -ArgumentList @($server, $web, $Port) -PassThru -WindowStyle Hidden
Start-Sleep -Seconds 2

try {
    $probe = Invoke-WebRequest -Uri "http://127.0.0.1:$Port/verify-card.html" -UseBasicParsing -TimeoutSec 10
    if ($probe.StatusCode -ne 200) { throw "harness did not serve (HTTP $($probe.StatusCode))" }

    # -----------------------------------------------------------------------
    # 3. Screenshot. Virtual time so the mocked timers actually fire.
    # -----------------------------------------------------------------------
    Write-Host '=== screenshotting ==='
    & $chrome --headless --disable-gpu --hide-scrollbars `
        --force-device-scale-factor=2 --virtual-time-budget=5000 `
        "--screenshot=$shot" "--window-size=$Width,$Height" `
        "http://127.0.0.1:$Port/verify-card.html" 2>$null | Out-Null
}
finally {
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
}

if (-not (Test-Path $shot)) { throw 'no screenshot was produced' }
Write-Host ''
Write-Host ("DONE  {0} bytes -> {1}" -f (Get-Item $shot).Length, $shot)
Write-Host ''
Write-Host 'Now LOOK at it. A byte count proves nothing about rendering.'
