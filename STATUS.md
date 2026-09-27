# Capybaras — STATUS

> **Entry point for every session.** Read this first. Update it last.
> I do not have continuous memory. This file is the memory.

**Last updated:** 2026-09-27 12:48 EDT
**Current phase:** **M3 — the shell runs. COMPLETE.** → **next: M4, the approval interface**
**Next milestone:** **M4 — the approval interface.** Plan: `docs/plans/next-steps.md`. Where the Replit gate meets a real UI
**Evidence so far:** `DECISIONS.md` D15–D18 · **Design:** `docs/plans/M4-visual-design.md`
**Recently decided:** D14 (no Bayesian maths in the safety path) · D15 (state directory) · D16 (supervision verified)
**Repo:** https://github.com/ThisJefferson/capybaras-desktop (public) · releases cut per milestone

---

## Now

- **Building:** Capybaras — a one-install, safe-by-default desktop agent that asks before it acts. A herd of capybaras, Rio de Janeiro flavour.
- **Ideal customer:** genuinely non-technical. Cloud API keys via OpenRouter OAuth. Free, donations, GitHub recognition.
- **The promise:** *it will still break, just small, visibly, and undoably.*

---

## Environment (measured 2026-09-27)

| Tool | Status |
|---|---|
| Node 24.16.0 / npm 11.13.0 | ✅ |
| **Rust — cargo 1.98.1 / rustc 1.98.1** | ✅ **installed this session via winget** |
| MSVC C++ Build Tools 2026 | ✅ already present |
| WebView2 runtime | ✅ already present |
| Git 2.55.0 | ✅ installed — **identity not yet set** |
| GitHub CLI 2.88.1 | ✅ installed — **auth in progress** |
| Python 3.14.7, Chrome, winget, WSL Ubuntu | ✅ |

**Note:** the current shell's PATH is stale; call Rust directly at `%USERPROFILE%\.cargo\bin\cargo.exe` until a new shell picks it up.

---

## Done

- `BUILD-PLAN.md` — packaging decision (Tauri + Node sidecar), phases, distribution, licensing, UI spec
- `BRAND.md` — name, the herd, the four states, Rio visual language, copy deck
- `mockup-main.svg` / `mockup-main.png` — interface mockup
- `EXECUTION-PLAN.md` — build order, workstreams, handoffs, acceptance criteria
- **Rust toolchain installed** — cargo/rustc 1.98.1
- **GitHub auth complete** as `ThisJefferson` (device flow, phone)
- **Git identity set** — `264281275+ThisJefferson@users.noreply.github.com` (noreply, real address never in history)
- **Repo created and pushed — M0 COMPLETE.** https://github.com/ThisJefferson/capybaras-desktop · public · 13 files · commit `62f2a78`
- **`DECISIONS.md` written** — D1–D10 recorded so settled questions don't get relitigated
- **Phase 1 complete: the risk classifier.** `src/risk-classifier/` — `tiers.ts` (four tiers, ordinal helpers), `classify.ts` (the classifier), `index.ts` (exports). Escalation-only; unknown tools fail safe.
- **99 tests green, typecheck clean.** `tests/classify.test.ts` — 84 cases across baseline tiers, blast radius, reversibility, egress, taint, destructive patterns, protected targets, remember-rules, and escalation-only properties. `tests/replit-acceptance.test.ts` — 15 cases.
- **The Replit acceptance test passes.** An agent with delete rights, told not to touch production, acting during a freeze: hard-gated, typed confirmation required, never rememberable, and the user is shown *every* reason — off-limits, 1,200 rows, irreversible.
- **Fixed a real bug the acceptance test caught** (D10): the classifier originally recorded a reason only when the tier moved, so a triple-hazard action reported a single reason. Tier and reasons are now computed independently.
- **Policy layer complete.** `src/policy/` — `grants.ts` (GrantStore), `gate.ts` (the gate), `index.ts`. Turns a classification into `proceed` / `ask` / `dry_run`.
- **141 tests passing, typecheck clean.** 3 suites: classify (84), policy (42), acceptance (15).
- **CI live and green** on every push, with a published test-results summary.
- **Community documents** — CONTRIBUTING, SECURITY, CODE_OF_CONDUCT, FUNDING, issue templates.
- **Brand assets** — 6 SVGs in `assets/brand/`, all valid, with coral correctly reserved to the two "needs you" files only.
- **Packaging + signing research** (31 KB) — found the Store account is now **free**, that MSIX gives instant SmartScreen trust, and that **MSIX has no Scheduled Tasks** (Gateway auto-start must become an MSIX Start-up Task). Recorded as D11; `BUILD-PLAN.md` §5 corrected.

## In flight

- Nothing blocked. Starting Phase 1.

## Blocked

| Item | Blocked on |
|---|---|
| CI workflow (`.github/workflows/ci.yml`) | `gh` token lacks the **`workflow`** scope. File is on disk, excluded via `.git/info/exclude`. Fix: `gh auth refresh -s workflow` (device code — phone-friendly, 30 seconds) |

## Not started

- Phase 1 — **the risk classifier** (critical path; next up)
- Phase 2 — Tauri + sidecar spike
- Parallel streams C (brand assets), D (docs/CI), E (packaging/signing prep)

---

## Next three actions

1. **M4 — build the approval interface.** Full plan in `docs/plans/next-steps.md`. The gap it closes: the shell currently **discards the sidecar's stdout** (`.stdout(Stdio::null())`) and speaks a one-command language, and **nothing yet runs the policy layer inside the sidecar**. So: versioned bidirectional IPC → run the gate in the sidecar → the approval round trip (the wait must be real and unbrypassable) → the card → durable grants → the herd → **the Replit acceptance test through the real UI**.
2. **The design system runs in parallel** — `docs/plans/M4-visual-design.md`. Tokens first (including the neutral ramp the card review flagged), typography, Lucide instead of hand-drawn icons, component kit, motion spec. Then the mascot model sheet: **one character, four poses, one sign** (D18).
3. Keep cutting a release per milestone with test results (Jeff's standing request).

---

## Phase 2 spike — result (2026-09-27)

`spikes/001-sidecar-supervision/` — code, harness and verdict. Question: does killing a supervisor orphan its Node sidecar, and what prevents it?

**Verdict: PARTIAL.** Clean shutdown works. But the question *cannot be fully answered from inside the agent's own process tree* — the host runtime anchors descendants with a Windows Job Object (`service-child-windows-job-anchor.js`), so there is no way to tell "the product would orphan" apart from "the harness killed it." The instrument contaminates the measurement.

**What the spike did establish, and it is actionable:**
- The mechanism that would have answered the spike **is the mechanism to build with** — a Job Object with `KILL_ON_JOB_CLOSE`. It is already a proven pattern in this exact stack.
- **`child.kill()` is not graceful on Windows** — it calls `TerminateProcess`, so no `SIGTERM` handler runs. A Gateway that owns sessions and sockets needs a handshake before the force.
- First harness was **confounded** (piped stdout) and produced contradictory output; rebuilt with `stdio: 'ignore'` + file logging. Bad instrumentation is not a finding.
- **MSIX full-trust — now tested, and it works.** See spike 002 below. (Jeff enabled Developer Mode, which unblocked it.)

---

## Phase 2 spike 002 — MSIX full-trust + bundled Node sidecar (2026-09-27)

`spikes/002-msix-sidecar/` — Rust launcher + bundled `node.exe` + bundled sidecar, packed to a 34 MB MSIX.

**Verdict: VALIDATED.** A full-trust MSIX app with package identity launched, resolved its app-local `node.exe`, spawned it, and Node ran: `node spawn: OK (exit 0)`, `sidecar ran under full-trust MSIX: v24.16.0`. This closes the highest-risk unknown from the packaging research.

**Two rules it imposes on the Gateway:**
- **Never resolve paths from `cwd`** — the packaged app launched with `cwd = C:\Windows\system32`.
- **Decide the state directory deliberately** — MSIX **silently virtualizes AppData writes**; the app's files landed in `%LOCALAPPDATA%\Packages\<PFN>\LocalCache\...` while `process.env.LOCALAPPDATA` still reported the plain path.

**Honest gap:** `app dir writable` read `yes`, but the test used a loose layout (writable by definition), not a real `WindowsApps` install. Re-take that reading when packaging begins.

**Install mechanics:** unsigned MSIX is rejected even in Developer Mode; machine-scope cert trust needs elevation; **loose-layout registration works unsigned and unelevated** and is the dev loop. Store re-signing means end users never touch a certificate.

---

## M3 — the shell: progress (2026-09-27)

`apps/desktop/` — Tauri v2 shell (`src-tauri/`), a Node sidecar (`sidecar/`), and a minimal status frontend (`web/`).

**Steps 1–6 complete, each verified:**

| Step | Result |
|---|---|
| 1. Toolchain | Tauri CLI 2.12.0; Rust `x86_64-pc-windows-msvc`; WebView2 153 |
| 2. Repo layout | `apps/desktop/`, shell imports the classifier rather than duplicating it |
| 3. Path discipline | `src/paths.rs` owns every lookup; **verified under `cwd = C:\Windows\System32`** |
| 4. State directory | D15 — explicit, overridable, and surfaced in the UI |
| 5. Sidecar supervision | Job Object with `KILL_ON_JOB_CLOSE` + graceful-stop handshake |
| 6. Non-orphaning, measured | **Hard-killed the shell from outside the process tree: zero orphans** |
| 7. Single instance, health, tray | **Second launch refused and exited cleanly; exactly one shell runs** |

**Tests:** 7 in the shell crate — 3 path, 1 single-instance, 3 supervision — all passing. The supervision test that matters asserts a graceful stop **reaches the sidecar's own handler**; on Windows a plain `Child::kill()` runs no handler, so it is a real assertion.

**Runtime verification of step 7:** launched twice. First instance ran with a heartbeating sidecar; second printed `Capybaras is already running (lock held at …)` and exited. One shell process, one sidecar, and both cleared on shutdown.

**Reproduce:** `cargo test` in `apps/desktop/src-tauri`; `apps/desktop/scripts/verify-supervision.ps1` for the outside-the-tree measurement.

---

## The acceptance test (memorise this)

> Reproduce the Replit incident of 23 July 2025 — an agent with delete rights, instructed not to touch production, acting during a code freeze — and confirm **Capybaras halts at the approval gate and cannot proceed without a human click.**

If Capybaras would have stopped the Replit incident, the product works. If not, nothing else matters.

---

## File map

```
projects/safe-agent-desktop/
  STATUS.md            <- you are here
  DECISIONS.md         <- why we chose things; read before relitigating
  BUILD-PLAN.md        <- what to build (packaging, phases, licensing, UI spec)
  BRAND.md             <- name, herd, states, palette, copy deck
  EXECUTION-PLAN.md    <- how it gets built: order, streams, handoffs
  mockup-main.svg|png  <- interface mockup
```

Related background:
```
research/safe-agent-blueprint.md              <- the architecture argument
research/agentic-harm-and-the-next-ai-winter.md <- the harm classes the classifier must cover
```

---

## Open questions

- Repo visibility — assumed **public** (donations + recognition). Confirm if wrong.
- Brazilian copy review — needs a human from Rio before 1.0. Not blocking.

## Standing notes

- **Never restart the Gateway** — owner-only. Queued config changes need Jeff's restart.
- Three hardening changes are written but inert until that restart: `fs.workspaceOnly`, qwen web deny, Telegram groups deny-by-default.
