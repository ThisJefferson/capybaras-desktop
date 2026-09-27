# Capybaras — STATUS

> **Entry point for every session.** Read this first. Update it last.
> I do not have continuous memory. This file is the memory.

**Last updated:** 2026-09-27 11:19 EDT
**Current phase:** Phase 1 ✅ **COMPLETE** (classifier + policy layer) → **next: Phase 2 spike**
**Next milestone:** M2 — the Tauri shell supervising a Node sidecar, *specifically inside an MSIX full-trust package*
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

1. **Phase 2 — the viability spike.** Throwaway prototype: does a Tauri window cleanly supervise a Node sidecar on Windows **inside an MSIX full-trust package**? This is the last unverified architectural assumption, and packaging depends on the answer.
2. Write the verdict in `DECISIONS.md` and stop if it is a no-go.
3. Fold any remaining stream output in; keep cutting a release per milestone with test results (Jeff's standing request).

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
