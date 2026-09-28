# Capybaras — STATUS

> **Entry point for every session.** Read this first. Update it last.
> I do not have continuous memory. This file is the memory.

**Last updated:** 2026-09-28 08:43 EDT
**Current phase:** **M5 — onboarding.** The PKCE flow, the credential store, the loopback listener, the session meter, the first model call, and now **the onboarding path walked end to end through the interface** are all in the tree and tested. What remains in M5 is a free-model mode, a decision about the spend cap (**D26** — there is no enforced cap; the account-balance readout is now wired, the cap itself is open), and the one thing no test can do — a real reply against the user's own account.
**Next milestone:** M5 — a non-technical tester reaches a first reply unaided (the path exists and is walked offline; a live key is the missing piece)
**Spec:** `docs/protocol.md` · **Plan:** `docs/plans/next-steps.md` · **Design:** `docs/plans/M4-visual-design.md` · **Onboarding test:** `docs/onboarding-test.md`
**Recently decided:** D19 (grants fail closed) · D20 (skills must declare) · D21 (the gate governs actions, not speech) · D22 (the meter pushes) · D23 (the card states each hazard once) · D24 (only a human's answer lowers the sign) · D25 (the model call lives in the shell) · D26 (the spend cap is display only — open; the balance readout is now wired)
**Repo:** https://github.com/ThisJefferson/capybaras-desktop (public) · releases cut per milestone

---

## Now

- **Building:** Capybaras — a one-install, safe-by-default desktop agent that asks before it acts. A herd of capybaras, Rio de Janeiro flavour.
- **Ideal customer:** genuinely non-technical. Cloud API keys via OpenRouter OAuth. Free, donations, GitHub recognition.
- **The promise:** *it will still break, just small, visibly, and undoably.*

---

## 2026-09-28 — the meter's readout is real, and it is display only

One bounded item: the "how much is left" row never rendered. The cause was not
missing arithmetic — `set_credit` was written and tested — but a missing **caller**:
nothing ever fetched a balance, so `Snapshot.credit` was always `None`. The row is
now reachable.

- **The missing reader.** `src/credits.rs` fetches `GET /credits` with the stored key
  and hands the balance to `meter::apply_credit`, which attaches it and pushes the
  same `capybaras://usage` event the meter already sends (D22) — so the interface
  needed no new listener. `renderUsage` already drew the row whenever a credit was
  present; it simply never had one.
- **Remote data, treated as remote.** The balance is money on a screen, so a shape
  that is not exactly understood — a missing field, a string where a number belongs,
  a negative or absurd figure, a non-2xx, an unreachable provider — produces
  **nothing**. The rule is the catalogue's: show nothing rather than something wrong.
- **A failed read cannot corrupt the meter.** `apply_credit(None)` changes no
  number: the last known figure stands, or the row stays absent. Asserted in Rust,
  and again through the interface — a refused call leaves the figure standing.
- **Two moments, both justified in comments.** The balance is read right after the
  model list (the first point the shell knows it holds a usable key, so the row
  arrives with the rest of the panel) and refreshed after each completed call (the
  one thing this app does that changes the balance). The choice is written at the
  call sites rather than left implicit.
- **What it is, said plainly on screen:** the **account** balance, shared with every
  other key on the account, labelled "left (whole account)". It is **not** a
  spending limit, and nothing is enforced with it.

**No limit was set, changed or removed** — the provider-side limit is still unset
(`limit: null`), which is the operator's decision. **D26 is otherwise unchanged**
and now carries a same-day amendment: the cap question is open, nothing enforces a
limit, and this readout is display only.

**Tests:** 366 TypeScript across 14 files (unchanged); **151 Rust, with 5
live-network probes still ignored** (was 140 + 5). `npm run verify` green — the
onboarding harness now asserts the row a person sees (`left (whole account)` at the
measured balance) and that a refused call leaves it standing.

**Honestly still unproven:**

- **The row in a running window.** This host cannot display one
  (`docs/DEBUGGING.md`), so what is proven is the interface rendering the row under
  the harness and the shell reading and pushing it under test. No human has seen it
  in the real Tauri window.
- **A live `/credits` read through the app's own path.** The endpoint was measured
  live by hand on 2026-09-28 and is pinned by an offline stub; the ignored
  `live_api.rs::probe_credits` remains the only live read, and it was **not run**.

---

## 2026-09-28 — the onboarding path is walked, and the cap question is settled

Two items. The first proves M5's exit as far as this host can; the second answers
the spend-cap question and records it rather than guessing.

- **M5's exit, through the interface.** `npm run verify:onboarding` drives the real
  `index.html`, `app.js`, `app.css` and `tokens.css` in headless Chrome over CDP,
  with the real sidecar behind them, and the Tauri/OS bridge as the only
  substitution — the pattern M4.7 set for the gate. 24 assertions, green. The
  bridge is now shared by both harnesses (`harness-bridge.js`); it only moves bytes.
  The scenario is documented in `docs/onboarding-test.md`.
- **Two real defects, found by asserting what a person needs and watching the
  assertion fail.** (1) A failed sign-in wrote its reason only to the developer
  log, so the panel still read "Not connected yet." — a button that appeared to do
  nothing. The reason now reaches the panel, in the shell's own words. (2) While
  the sign-in ran, nothing on the panel said so; it now says a browser window has
  opened and to come back. Both fixed in `app.js`, not in the test.
- **The seam is asserted, not assumed.** The check reads the frontend and the
  shell's Rust source and fails if the page listens for an event the shell never
  mentions, or invokes a command the shell does not register (16 events, 10
  commands). Copy-pasted names can no longer drift silently.
- **No billable call.** Both OpenRouter calls the flow needs are answered from
  fixtures; the live probe stays `#[ignore]`d.

**The spend cap: display only — and not even that is wired.** No file enforces a
limit: `chat::call` sends with no budget check (`apps/desktop/src-tauri/src/chat.rs:84`),
`usage.rs` only accumulates and formats, and the `app.js` references are display.
The one refusal that exists is the provider's own 402, relayed as a plain sentence.
Worse, the display is unreachable: `set_credit` (`meter.rs:54`) has **no caller**, so
`Snapshot.credit` is always `None` and the "left" row never renders. Nothing was
implemented, because M5 §4 specifies a *provider-side* limit and there is no spec
for the rest — recorded as **D26** with the evidence and what a decision must cover.

**Tests:** 366 TypeScript across 14 files (unchanged); 140 Rust, with 5 live-network
probes ignored (unchanged). `npm run verify` green — it now includes
`verify:onboarding`; `verify:ui` and `verify:acceptance` green.

**Honestly still unproven:**

- **A first reply against the real OpenRouter account.** Unchanged, and not done by
  instruction — it spends money. Connect → sign in → Send on an unlocked desktop, or
  the ignored `live_api.rs` probe.
- **The window.** No human has looked at the running Tauri window. This host cannot
  display one (`docs/DEBUGGING.md`).
- **The seam is asserted structurally, not executed.** The onboarding harness proves
  the interface and asserts the command/event names against the shell; it does not
  run the Rust path, and the payload field names (`models`/`default`, `text`/`model`)
  are the one join neither side re-derives from the other. Closing it properly needs
  the window this host cannot show.
- **The spend cap.** Nothing enforces one, and the app does not show the one that
  exists. See D26.

---

## 2026-09-28 — the first model call, and a reply can be reached

The gap this file named at 02:47 — *"there are no model calls in the app yet"* — is
closed. One bounded item: the smallest honest path from a connected key to a first
reply.

- **The shell makes the call.** `src/http.rs` (the transport seam), `src/catalog.rs`
  (the model list), `src/chat.rs` (one completion). Rust — not the sidecar and not
  the frontend — because the key is authority and must not cross the protocol
  stream (T12, M5-onboarding.md §3). Recorded as D25.
- **The catalog follows `models.ts` rather than inventing a convention.** The same
  bounds (80 / 240), the same control-character flattening, the same
  display-fields-only guarantee, the same "an empty catalog is an error, not an
  empty menu". The structural half is asserted in **both** languages, so the
  property that keeps model choice off the gate cannot drift silently.
- **One completion, bounded.** One request, one reply, `max_tokens` 256, prompt
  capped at 4,000 characters. No history. No streaming — and absent on purpose:
  `usage.rs` names the trap where a stream without a usage block reads as zero.
- **The meter's seam is now exercised by a real caller.** `chat::perform` reports
  every finished call through one closure, and that closure is
  `record_model_call`. A **failed** call reports zeros rather than nothing (D22) —
  asserted rather than assumed, by driving `report_of` → `record_and_notify` against
  a stub.
- **Errors are plain sentences.** No status code, no provider body, no interpolated
  transport error reaches the screen — swept across thirteen refusal statuses and a
  transport failure. The out-of-credit sentence gained a next step because the test
  demanded one (it originally stated the cause and left the reader there).
- **The interface renders the list and the reply with `textContent` only**, and the
  model list is loaded automatically once a key is present, so reaching a reply does
  not depend on discovering that a "Load models" button exists.

**Tests:** 366 TypeScript across 14 files (unchanged); **140 Rust, with 5 live-network
probes ignored** (was 119 with 4). `npm run verify` green; `npm run verify:ui` passed
(6 agents, card present, tokens resolved); `npm run verify:acceptance` green — the
card, the herd, the wait, the click, and Enter-is-safe all unchanged by the new
panels.

**What this proves, and how — without spending anything.**

- A first reply is now **reachable in the code path**: connect → the shell loads
  `/models` with the stored key → the interface shows the list → Send runs one
  completion → the reply is shown and the meter moves.
- **It was NOT proven by a live billable call.** Every model-call test runs offline
  against a stub. The one live probe
  (`live_api.rs::probe_the_first_reply_through_the_apps_own_path`) is `#[ignore]`d and
  **was not run**. So what is proven is the request that gets built, the parsing of a
  realistic reply, the plain handling of every failure, and the meter moving — not
  that OpenRouter accepts a real key today.

**Honestly still unproven:**

- **A first reply against the real OpenRouter account.** It needs a connected key and
  spends a small amount; not done here, by instruction. Either run
  `cargo test --test live_api -- --ignored --nocapture` in `apps/desktop/src-tauri`,
  or click Connect → Load models → Send in the app.
- **The window.** `verify:ui` and `verify:acceptance` are green with the new panels in
  place, but no human has looked at the running Tauri window. That is M4's oldest open
  item and it is unchanged.
- **"One call at a time" is the interface's guard, not the shell's.** The Send button
  is disabled while a call is in flight; the shell holds no lock, so a hostile page
  able to invoke twice could start two billed calls. Cheap to close when the Tauri
  capability scoping in T3 is done.

---

## 2026-09-28 — the interface is looked at, and the gate is driven through it

Three milestone items. Two were already built; the audit found two real defects.

- **M4.4 — the interface renders, and was reviewed.** `npm run verify:ui` passes
  (six agents, card present, tokens resolved) and its frame was reviewed with a
  vision model: the headline, all three reasons, the hard-gate callout, the typed
  confirmation, and "Go ahead" disabled. The reviewed frame is
  `assets/design/interface-rendered.png`.
- **M4.7 — the acceptance test, through the interface.** `npm run verify:acceptance`
  drives the documented scenario through the real frontend — real `index.html`,
  `app.js`, `app.css`, `tokens.css`, and the real sidecar — with headless Chrome
  over CDP. 22 assertions, green, including: the card halts and states what it
  intends; "Go ahead" is disabled until the phrase is typed; nothing runs across
  2.5 s of waiting; an explicit click does proceed; and **Enter holds on rather
  than approving**. The reviewed frame is `assets/design/acceptance-halt.png`.
- **M4.6 — the herd was already built and tested** (`508b53f`). The audit found
  the state machine could produce the WRONG visible state: a pending approval's
  sign was lowered by that same agent's own background work. Fixed, with both new
  assertions written first and watched to fail.

**Two defects found by doing M4.4 and M4.7, both on the milestone card:**

- the card stated the same hazard twice — five reasons, two of them restatements.
  It now shows exactly the three `docs/acceptance-test.md` documents.
- the herd could contradict the card (the sign defect above).

**Honest limit, recorded in `docs/DEBUGGING.md` rather than implied:** a screenshot
of the real Tauri window is **not possible on this host**. The console session is not
compositing — `CopyFromScreen` returns a uniform black frame, a magenta canary
window was present in the window list yet contributed zero pixels, and the centre
pixel read pure black. A black capture is indistinguishable from a blank window, so
it proves nothing. The offscreen render is the check that can be made, and the one
thing still unproven is the **Rust shell plus WebView2 window** — not the interface
and not the gate.

**Tests:** 366 TypeScript across 14 files; 119 Rust, with 4 live-network probes
ignored. `npm run verify` green.

---

## 2026-09-28 — the meter reaches the interface, and grants become revocable

Two pieces of wiring, both previously "exists but unreachable".

- **M5 — `usage.rs` is now read.** `UsageMeter` holds the session; `usage_status` reads it; `record_model_call` is the single seam a finished model call reports through, and it emits `capybaras://usage` for **every** call — not only the ones that move a total, because a request that fails after the provider began generating returns no usage, and that standstill *is* the signal (D22). The interface reads the meter once on load and follows the event after that. `usage.rs`'s public API is unchanged: the arithmetic was fine, the wiring was missing.
- **M4.5 — remembered choices are revocable from the interface.** Persistence already landed (`48e5633`, D19); what was missing was a caller. Added `list_grants` / `revoke_grant`, plus a plain list with a one-click *Forget*. The proof is behavioural, not declarative: `tests/protocol.rs::a_revoked_grant_asks_again` revokes a grant and then makes the same action **ask again**.

**Tests at that point:** 364 TypeScript across 14 files; 118 Rust, with 4 live-network probes ignored. `npm run verify` green.

**Honest gaps, stated rather than implied:**

- There were **no model calls in the app yet** at that point, so the seam and its tests
existed ahead of the first caller — which is what made the first model call impossible to
write without one. **Superseded later the same day: see "the first model call" above.**
- The new panels are covered by typecheck, the DOM smoke test and `check-tokens` — **not** by a screenshot. M4.4's "look at it in a real window" item is still open, and it is now the oldest open item in M4.

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

- **The meter's readout — the "left" row is reachable.** `src/credits.rs` reads
  `GET /credits`, `meter::apply_credit` attaches it, and the interface renders
  "left (whole account)". Display only; no limit touched (D26, amended).
- **M5's onboarding path — verified through the interface.** `npm run verify:onboarding`
  (24 assertions, green), the two defects it found fixed, and the scenario documented
  in `docs/onboarding-test.md`. The spend cap answered honestly as D26.
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

- **M5 — onboarding.** The credential store, the OAuth PKCE flow, the model catalog (fetched, sanitised and rendered), the loopback listener, the token exchange, the session meter, the first model call, **and the onboarding path verified end to end through the interface** (`npm run verify:onboarding`) are all in the tree and tested. Remaining: a free-model mode, a decision on the spend cap (D26), and a live first reply.
- **M4's human step — the only one left.** A person looking at the running app in a real window. Rendering and the scenario are proven offscreen now (`npm run verify:ui`, `npm run verify:acceptance`); what this host cannot do is display a window at all.

## Blocked

| Item | Blocked on |
|---|---|
| Nothing | — — the CI-workflow `workflow`-scope block is resolved; `.github/workflows/ci.yml` is tracked and running |
| A screenshot of the **real app window** | This host's console session is not compositing — `CopyFromScreen` returns black. Not a code problem: it needs an unlocked, interactive desktop. |

## Not started

- **M6 — packaging** (MSIX, bundled Node runtime, clean install *and* uninstall). Carries spike-002's open readings: re-take `app dir writable` against a real `WindowsApps` install, and answer the MSIX state-path question (D15).
- **M7 — signing and distribution**; **M8 — hardening and audit.**
- **Design stream:** typography, adopting Lucide, the motion spec, and the mascot model sheet — one character, four poses, one sign (D18).

---

## Next three actions

1. **A live first reply (M5).** The path is built, walked offline and verified
   through the interface; what is missing is one real key and one real call —
   Connect, sign in, Send. Then a free-model mode, and a decision on the spend cap
   (D26: nothing enforces one today; the app now shows the account balance, but that
   is a readout, not a cap).
2. **Look at the real window, and click the scenario by hand.** The offscreen checks pass and the defects they found are fixed; what remains is a person on an unlocked desktop running `run.cmd` and clicking through the gate — which is the milestone gate itself.
3. **Keep cutting a release per milestone with test results** (Jeff's standing request).

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
