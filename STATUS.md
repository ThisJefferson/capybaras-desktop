# Capybaras — STATUS

> **Entry point for every session.** Read this first. Update it last.
> I do not have continuous memory. This file is the memory.

**Last updated:** 2026-09-27 10:36 EDT
**Current phase:** Phase 0 — Unblock and scaffold
**Next milestone:** M0 (unblocked + scaffolded)

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
- **Rust toolchain installed**
- **GitHub device-code login initiated** (code issued, awaiting user authorisation)

## In flight

- GitHub auth — waiting on Jeff to enter the device code from his phone
- Git identity — will be set from the authenticated GitHub username (noreply email)

## Blocked

| Item | Blocked on |
|---|---|
| Repo creation | GitHub auth |
| First commit | Git identity (needs auth) |
| Everything downstream of M0 | the above two — both minutes away |

## Not started

- Phase 1 — **the risk classifier** (critical path; next up)
- Phase 2 — Tauri + sidecar spike
- Parallel streams C (brand assets), D (docs/CI), E (packaging/signing prep)

---

## Next three actions

1. Wait for GitHub auth → set git identity → `git init` → scaffold → first commit → **M0**
2. **Start Phase 1: the risk classifier, headless, with tests.** Includes the Replit acceptance test.
3. Open streams C, D, E as subagents.

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
