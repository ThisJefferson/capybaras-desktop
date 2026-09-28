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
#         [-Port 8791] [-Width 940] [-Height 1520]
#
# Height note: the interface grew past the card (the model panel, the meter and the
# remembered choices all sit above it now), so the old 1060 window CLIPPED the card's
# confirmation field and buttons -- the two things this check exists to see. 1520 shows
# the whole card in one frame.

param(
    [int]$Port = 8791,
    [int]$Width = 940,
    [int]$Height = 1520
)

# Chrome writes progress text to stderr. Under 'Stop', PowerShell treats a native
# command's stderr as a TERMINATING error -- which killed this script at the
# screenshot line. It printed a successful screenshot and then exited 1 without
# ever running its own assertions. Relaxed deliberately: every failure this
# script cares about is an explicit `throw`, and those still stop it.
$ErrorActionPreference = 'Continue'

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

# The server lives IN THE REPO. It used to point at a scratch file outside it,
# which meant this script -- the smoke test -- could only run on the machine it
# was written on, and so could never run in CI.
$server = [System.IO.Path]::GetFullPath((Join-Path $here '..\..\..\scripts\serve-static.mjs'))

# Chrome in a few plausible places, because hard-coding one path is how a check
# silently stops running on a different machine.
$chromeCandidates = @(
    'C:\Program Files\Google\Chrome\Application\chrome.exe',
    'C:\Program Files (x86)\Google\Chrome\Application\chrome.exe',
    (Join-Path $env:LOCALAPPDATA 'Google\Chrome\Application\chrome.exe'),
    '/usr/bin/google-chrome',
    '/usr/bin/chromium'
)
$chrome = $chromeCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $chrome) { throw 'Chrome not found. Tried: ' + ($chromeCandidates -join ', ') }
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
  // THE REAL v2 SHAPE, deliberately. Tauri v2 exposes invoke under `core`.
  // An earlier mock put it at the top level (the v1 shape) -- which is why the
  // smoke test passed while the real app failed on every button. A mock must
  // mirror the API, not the assumption.
  core: {
    invoke: async function (cmd) {
      if (cmd === 'shell_status') {
        return { shell_pid: 4242, state_dir: 'C:\\Users\\Skept\\AppData\\Local\\Capybaras',
                 sidecar_running: true, sidecar_pid: 5150, job_assigned: true };
      }
      // The mock must mirror the REAL command surface, not just the commands this
      // screenshot happens to depend on. Returning null for `connect_status` made
      // the interface render a TypeError where the real app renders a sentence --
      // a harness artefact that reads as a product defect in a reviewed image.
      if (cmd === 'connect_status') {
        return { store_available: true, connected: false };
      }
      if (cmd === 'usage_status') {
        return { calls: 3, total_tokens: 12480, cost_display: '$0.0041',
                 credit: { scope: 'account', remaining_display: '$8.60', remaining_usd: 8.6 } };
      }
      return null;
    }
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
// Probe: record the RESOLVED value of the semantic tokens into the DOM, so a
// text dump can prove the stylesheet actually loaded AND resolved. This is the
// exact defect that got past every other check: tokens that exist in the file
// but resolve to nothing.
window.addEventListener('load', function () {
  setTimeout(function () {
    var style = getComputedStyle(document.documentElement);
    document.body.setAttribute('data-probe-primary', style.getPropertyValue('--color-semantic-state-primary').trim());
    document.body.setAttribute('data-probe-coral', style.getPropertyValue('--color-semantic-state-needsYou').trim());
  }, 400);
});
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

    # -----------------------------------------------------------------------
    # 4. Assert on the live DOM. A file appearing proves nothing; these check
    #    that the script ran, the herd rendered, the card rendered, and the
    #    tokens RESOLVED.
    # -----------------------------------------------------------------------
    Write-Host '=== asserting on the rendered DOM ==='
    $dom = & $chrome --headless --disable-gpu --hide-scrollbars `
        --virtual-time-budget=5000 --dump-dom `
        "http://127.0.0.1:$Port/verify-card.html" 2>$null | Out-String
}
finally {
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
}

if (-not (Test-Path $shot)) { throw 'no screenshot was produced' }
if ([string]::IsNullOrWhiteSpace($dom)) { throw 'no DOM was dumped -- the page did not load' }

$failures = @()

# The herd: six agents, all present.
$agents = ([regex]::Matches($dom, 'data-agent=')).Count
if ($agents -ne 6) { $failures += "expected 6 agents in the herd, found $agents" }

# The card actually rendered, with a headline.
if ($dom -notmatch 'card__headline') { $failures += 'the approval card is missing from the DOM' }

# The tokens RESOLVED. Empty means the stylesheet loaded but the variables did
# not, which is the silent failure this whole script exists to catch.
# Probe values, extracted with simple string splits.
#
# Deliberately NOT regex, and deliberately not double-quoted strings containing
# quotes. The previous version used \" inside a double-quoted PowerShell string,
# and backslash is NOT an escape character in PowerShell -- so the strings
# terminated early and the whole script failed to parse. A split cannot go wrong
# that way.
$primary = ($dom -split 'data-probe-primary="') | Select-Object -Skip 1 -First 1
if ($primary) { $primary = ($primary -split '"') | Select-Object -First 1 }
$coral = ($dom -split 'data-probe-coral="') | Select-Object -Skip 1 -First 1
if ($coral) { $coral = ($coral -split '"') | Select-Object -First 1 }

if ([string]::IsNullOrWhiteSpace($primary)) {
    $failures += 'data-probe-primary resolved to nothing -- the token is missing or undefined'
}
if ([string]::IsNullOrWhiteSpace($coral)) {
    $failures += 'data-probe-coral resolved to nothing -- the token is missing or undefined'
}

if ($failures.Count -gt 0) {
    Write-Host ''
    Write-Host 'SMOKE TEST FAILED:'
    foreach ($f in $failures) { Write-Host ('  - ' + $f) }
    throw 'the rendered interface did not pass its assertions'
}

Write-Host ''
Write-Host ('PASSED  ' + $agents + ' agents, card present, tokens resolved (' + $primary + ' / ' + $coral + ')')
Write-Host ('        ' + (Get-Item $shot).Length + ' bytes -> ' + $shot)
Write-Host ''
Write-Host 'Now LOOK at it too. A passing assertion is not a judgement about design.'
