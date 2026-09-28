# Debugging plan

**Read this before debugging anything in this repository.** It is not general advice — every item comes from a real defect found in this project, and several of them cost an hour or more.

The pattern worth internalising: **almost every bug in this codebase so far has been a *silent* failure.** Nothing threw. Everything looked nearly right. That is what this document is for.

---

## 1. The three failure classes

Every significant bug in this project has fallen into one of these. Learn the shape and you will spot the next one faster.

### Silent failures — by far the most dangerous

The code runs, nothing errors, and the output is quietly wrong.

**Example: the token bug.** `design/tokens.json` referenced its sources with paths missing their root (`"ref": "brand.atlantic"` instead of `"color.brand.atlantic"`). The generator's lookup returned `undefined` and the walker **skipped the token without complaint**. Result: all 15 semantic tokens missing from the stylesheet. The card rendered with no coral, no filled button, no borders — *structurally* fine, visually broken.

A stylesheet with missing values is worse than one that fails to build, because the UI degrades quietly and looks almost right.

**The fix pattern: make it loud.** `build-tokens.mjs` now collects unresolved references and **exits non-zero**, naming them. Any time a lookup can fail silently, make it fail the build instead.

### Invalid instruments — the harness lied

**Example: the `file://` harness.** The first attempt to verify the approval card loaded `index.html` from disk. Chrome **refuses to load ES modules over `file://`** (CORS, origin `null`), so `app.js` never executed. The screenshot showed an unstyled shell — and proved *nothing* about the code. It looked like a product bug. It was a harness bug.

**The fix pattern: verify the instrument before trusting its output.** If a test says something is broken, confirm the test can detect a *working* case first.

### Wrong verification method — measuring the wrong thing

**Example: the PDF tables.** A text-extraction pass reported "both tables broken" in the paper. Text extraction *flattens* tables by design; the tables were perfectly fine. The check was measuring the extractor, not the document.

**The fix pattern: match the method to the claim.** Rendering claims need rendering checks — a screenshot, reviewed. Byte counts and text extraction prove nothing about layout, colour or legibility.

---

## 2. Golden rules

1. **Verify rendering visually.** Never trust a byte count, an exit code, or extracted text to tell you something *looks* right.
2. **Fail loudly.** A silent skip is a future bug. Make builds refuse bad input.
3. **One source of truth, generated.** `tokens.json` → `tokens.css`; `sidecar/src` → `sidecar/dist`. Never hand-sync what can be generated, and never edit a generated file.
4. **Do not reach for clever one-liners on files.** Literal operations beat regexes. (See §4 — this has bitten three times.)
5. **Check the instrument.** Confirm your harness can see a known-good case before believing it about a bad one.
6. **Fix the cause, not the symptom.** When a fix does not work, **find out why before trying again.** The Duda bug needed two attempts because the first fix was silently overridden — see §4.

---

## 3. Layer-by-layer playbook

### Frontend (the Tauri webview)

**You cannot check this with `file://`.** Serve it over HTTP:

```powershell
node .openclaw/tmp/serve-static.mjs apps/desktop/web 8791
# then point Chrome at http://127.0.0.1:8791/verify-card.html
```

There is a verification harness that injects a mock `window.__TAURI__` before `app.js`, so the **real** `app.js` and **real** `app.css` render a real card and herd. Regenerate it from `index.html`; it is gitignored.

Then screenshot and **review it with a vision model** — the usual script here is prompted for phone screenshots and will describe the wrong things, so ask a specific question instead.

**A real window cannot be captured on this host — measured, 2026-09-28.** The console session is not compositing (a locked/disconnected desktop). `Graphics.CopyFromScreen` returns a **uniform black frame**: a magenta canary window was confirmed present in the window list (alongside Notepad and Windows Terminal) while contributing **zero** magenta pixels to the capture, and the centre pixel read `ARGB=-16777216` (pure black). So "screenshot the running app's window" is not a check this machine can make — and a black screenshot must not be mistaken for a blank window in the product. Verify the interface **offscreen** instead:

```powershell
npm run verify:ui          # renders the real app.js + app.css in headless Chrome
npm run verify:acceptance   # drives the REAL sidecar through the REAL frontend
```

Both are rendering checks; neither needs the desktop. They substitute the Tauri/OS bridge (a separate script injected before `app.js`, impossible to inline under `script-src 'self'`) and leave the product code above it — `index.html`, `app.js`, `app.css`, `tokens.css` and the whole sidecar — unmodified.

### Protocol (shell ↔ sidecar)

- **`stdout` is the protocol channel.** Human-readable diagnostics go to `sidecar.log`. Writing free text to `stdout` corrupts the stream — this is the first thing to check if messages look garbled.
- The protocol is **newline-delimited JSON, one object per line**. Read `docs/protocol.md`.
- `agents.state` is **whole state, not a delta** — a dropped message self-corrects on the next one.
- Rust tests have a `drain()` helper for cases where **message order matters**. `expect()` skips non-matching messages, which will silently discard an `agents.state` that arrives just before an `approval.required`.

### Sidecar

```powershell
# The log file is the human view. Tail it while driving the protocol.
Get-Content "$env:LOCALAPPDATA\Capybaras\sidecar.log" -Wait
```

Grant state lives at `$env:LOCALAPPDATA\Capybaras\grants.json` and **fails closed** — delete it to reset, and remember that a corrupt file yields an *empty* store, not an error.

### Rust shell

- **`cargo test` needs the sidecar bundle built first** (`npm run build:sidecar`), or the path assertion fails.
- `apps/desktop/scripts/verify-supervision.ps1` re-takes the outside-the-process-tree checks (orphans, `cwd` independence). It launches via a Scheduled Task deliberately — an in-tree measurement is contaminated by this host's own Job Object.

### Tokens

```powershell
npm run build:tokens   # fails loudly on an unresolved ref
```

---

## 4. Known landmines

Check these first. Each one has already caused a defect or an hour of confusion.

| Landmine | Symptom | Reality |
|---|---|---|
| **CSP is `script-src 'self'`** | Inline `<style>`/`<script>` silently ignored | Styles and scripts must be **external files** |
| **ES modules over `file://`** | Blank or half-rendered UI | CORS blocks them; serve over HTTP |
| **CSS specificity vs source order** | An override "does not work" | Equal specificity → **later in the file wins**. Raise specificity instead of relying on order |
| **`tokens.json` refs** | A token silently missing | Refs must be **full dotted paths** from the root (`color.brand.sand`) |
| **Generated files** | Edits vanish | `tokens.css` and `sidecar/dist/` are generated — edit the source |
| **`stdout` in the sidecar** | Protocol messages garbled | `stdout` is the protocol; log to `sidecar.log` |
| **Rust tests before the bundle** | Path assertion fails | Run `npm run build:sidecar` first |
| **MSIX virtualises AppData** | State "disappears" | Writes may land in `Packages/<PFN>/LocalCache` (D15) |
| **Windows Job Objects** | Unexpected process death | A parent job can terminate children; nested jobs can be refused |

### PowerShell traps — three separate incidents

1. **`Get-Content -Raw` reads with the system ANSI codepage**, mangling UTF-8. Every em-dash in a generated document was corrupted before `pandoc` ever saw it. **Use `[System.IO.File]::ReadAllText($path, [System.Text.Encoding]::UTF8)`.**
2. **`[^\\n]` in a `-replace` is NOT "not newline".** In .NET regex it means *"not backslash or n"* — so it stopped at the first `n` in "interface" and mangled a file header.
3. **Backticks in a double-quoted string are escape characters.** A commit message containing `` `.stdout()` `` broke the whole command. **Use `git commit -F <file>` for any message with backticks or quotes.**
4. Also: **`.NET` relative paths resolve against the *process* directory, not PowerShell's location.** Always pass absolute paths.
5. **`Join-Path` takes only TWO positional arguments** (`-Path`, `-ChildPath`). `Join-Path $a '..' 'web'` throws *"A positional parameter cannot be found"*, the script dies on that line, and whatever it was going to do silently does not happen. Nest the calls, or use one path string. This cost a stale-harness false report.
6. **A script that produces no output probably failed.** When a filtered result set comes back *empty*, read the unfiltered output before concluding anything — a filter that matches nothing looks identical to a clean run.

---

## 5. Verification checklist

Before claiming anything works, answer these. If a line does not apply, say why.

- [ ] Do the tests actually **test the new behaviour**, or do they test something adjacent?
- [ ] Is there a test for the **failure** direction, not just the happy path? (Especially: does anything fail *open* that should fail closed?)
- [ ] Has anything **rendered** been looked at, rather than measured?
- [ ] Did I confirm the **instrument** can detect a working case?
- [ ] Are generated files rebuilt **from source**, not hand-edited?
- [ ] Did I verify my own edit landed **and was not overridden** (CSS, config, generated output)?
- [ ] If I wrote a fix and it did not work, did I **find out why** before trying again?

---

## 6. Planned instrumentation

Concrete work, in rough priority order. None of it is exotic; all of it would have caught a bug above.

1. **CI must run the generators, not just the tests.** `build:tokens` (with its validation) and `build:sidecar` before `typecheck` and both suites. A CI run that skips the generator cannot catch a broken token ref.
2. **A hidden review route** rendering every component in every state on one page — the card, both tiers, the herd in all four states, empty/loading/error. Screenshot it in one shot instead of hunting for states by hand.
3. **A launch smoke test.** Start the app headlessly, screenshot it, and fail if the window is blank. This is the check that would have caught the `file://` class of problem immediately.
4. **A strict-tokens lint.** Parse `tokens.css` and fail on any `var(--x)` used in CSS that was never emitted. This would have caught the missing semantic tokens *at build time* rather than in a screenshot.
5. **Encoding checks on documents.** Verify generated Markdown/HTML is UTF-8 with no mojibake markers (the synthesis build script already carries a canary for this).
6. **Visual regression snapshots** of the review route, so a future change that breaks a colour or a layout shows up as a diff rather than as a user complaint.

---

## 7. Where the risk actually is

Ranked by how likely each is to produce the next nasty bug:

1. **Silent degradation in generated output** — the token class. Mitigate with validation that exits non-zero.
2. **The frontend at runtime.** The CSP/`file://` combination means a mistake produces a *blank window* rather than an error. The smoke test in §6.3 is the answer.
3. **MSIX virtualisation** (D15). Untested, and it moves files the code believes it knows the location of.
4. **Anything measured rather than looked at.** Every time this project has trusted a number over a rendering, the number has been wrong.

---

*Last updated 2026-09-28, after the token, `file://`, mojibake, CSS-specificity and regex-defect incidents, and the display-environment finding above. Add to it when something costs you an hour — that is the whole point of the file.*
