# Next steps — the road from here

Compiled 2026-09-27. Answers: *what happens next in the build?*

---

## Where we are

| Milestone | State |
|---|---|
| **M0** Unblocked + scaffolded | Done |
| **M1** Classifier complete | Done — 141 TypeScript tests |
| **M2** Spike verdicts | Done — spike 001 PARTIAL, 002 VALIDATED, 003 on mascots |
| **M3** Shell runs | **Done** — 7 Rust tests, released as `v0.0.3-alpha` |
| **M4** The approval interface | **In progress** — M4.1–M4.3 done (`51c351a`); M4.4–M4.7 remain |
| **M5** Onboarding | Not started |

**What exists:** a risk classifier and policy layer with 148 passing tests across both languages; a Rust shell that supervises a Node sidecar with measured lifecycle guarantees; brand identity; design tokens pending; an approval-card design that has been reviewed.

**What does not exist: the agent doing anything.** The gate exists in code and in tests, but **no interface has ever asked a human a question**. That is the whole of M4.

---

## M4 — the approval interface

**Why it is next:** it is the phase where the product becomes itself. Everything built so far is infrastructure for this moment.

**The acceptance gate, unchanged:**

> Reproduce the Replit incident of 23 July 2025 — an agent with delete rights, instructed not to touch production, acting during a code freeze — and confirm that Capybaras halts at the approval gate, states plainly what it intends to do, and **cannot proceed without a human click.**

---

### The gap M4 closes — and it is plumbing nobody will ever see

The classifier and policy layer are TypeScript in `src/`. The shell is Rust supervising a Node sidecar. **Nothing connects them.** Measured on the current code:

1. **The shell discards the sidecar's output.** `sidecar.rs` spawns with `.stdout(Stdio::null())`. The sidecar literally cannot talk back — and the protocol is one-way: the shell sends, the sidecar obeys.
2. **The protocol has exactly one command.** Newline-delimited JSON on stdin, and the only recognised message is `{"cmd":"shutdown"}`.
3. **Nothing runs the policy layer inside the sidecar.** `package.json` has `test`, `typecheck` and `replit-test` — no build or bundle step. The gate has never executed outside a test runner.

Those three are the real work of M4. Everything else is presentation.

---

### Steps

**M4.1 — Define the IPC protocol, versioned.**
Replace the one-command channel with a framed, bidirectional JSON protocol: `action.proposed` → `gate.decided` → `approval.requested` → `approval.answered`. Pipe the sidecar's stdout and read it. Include a message version field so the two halves can evolve without silent breakage.
**Exit:** a documented protocol plus a test that round-trips every message type.

**M4.2 — Run the gate in the sidecar.**
Execute the policy layer inside the Node sidecar. Sorting out how TypeScript runs there is the first practical question — Node 24 can strip types natively, which may avoid a build step entirely; verify rather than assume.
**Exit:** an action proposed over IPC returns a tier and its reasons from the real classifier.

**M4.3 — The approval round trip, and the wait must be real.**
When the gate returns `ask`, the sidecar blocks and waits for a decision. **The agent cannot proceed while waiting, and the shell has no path that auto-approves.** If the wait can be bypassed, the product's central promise is void — so this gets a test that tries to bypass it.
**Exit:** a `confirm`-tier action blocks until answered, and a bypass attempt fails.

**M4.4 — Render the approval card.**
The reviewed design becomes a real component, at real size, showing **every** reason. Built on the design tokens, not retrofitted to them.
**Exit:** the card renders a real approval request from the sidecar, with all reasons, in plain language and no tool identifiers.

**M4.5 — Grant persistence and revocation.**
The grant store becomes durable. This is where the state-directory decision (D15) stops being theoretical — grants must survive a restart, and under MSIX that path may be virtualised.
**Exit:** a grant survives a restart, satisfies exactly one action against one target, expires, and can be revoked. **A hard gate is still never satisfied by memory.**

**M4.6 — The herd, in four states, driven by real state.**
Six instances of one character, distinguished by label, accent colour and position (D18). Each state maps to something the sidecar actually reports — not a decorative animation.
**Exit:** a state change in the sidecar produces the correct visible state.

**M4.7 — The acceptance test, end to end, through the interface.**
The Replit scenario, run against the real app with a real person clicking. The existing `tests/replit-acceptance.test.ts` proves the logic; this proves the product.
**Exit:** the documented scenario halts, states plainly what it intends to do, and cannot proceed without a click. **This is the milestone gate.**

---

### Progress, and the gaps M4.4–M4.7 actually have to close

**M4.1–M4.3 are done and verified** — commit `51c351a`, 153 tests passing. Measured against the current code, here is what genuinely remains. **Three of the four are not the card.**

**M4.4 — the card needs a push channel that does not exist.**
The shell's only route to the interface today is a **pull-based** Tauri command (`shell_status`), and the frontend polls it every two seconds. There is **no `emit`, no event, no way for the shell to tell the UI that something needs a human.** Worse: **nothing consumes the protocol channel** — `recv_json` exists in `sidecar.rs` but is called only from tests, so in the real app the sidecar's messages currently go nowhere at all.
So M4.4 is three things in order: a reader loop that turns protocol messages into Tauri events, a command to answer an approval, and *then* the card.
**Exit:** a real `approval.required` from the sidecar appears in the UI without polling, showing every reason.

**M4.4b — the tokens the card is supposed to be built on do not exist.**
`design/tokens.json` is absent; there are no stylesheets anywhere in the project. Starting the design system is therefore a **prerequisite**, not a parallel nicety. Tokens first, card second — otherwise the card gets hand-styled and quietly becomes the thing everything else is retrofitted around.

**M4.5 — grants do not persist.**
`grants.ts` is an in-memory `Map` with no read or write anywhere in the codebase. Making it durable is a small change with a **large consequence**: it is what raises D15's MSIX state-path question from theory to a blocking decision, because the file has to live somewhere MSIX may virtualise.

**M4.6 — the protocol has no message for agent state.**
"The herd, driven by what the sidecar actually reports" cannot start until the protocol carries that report. A new message type is needed, plus the state machine behind it. Small, but it is genuinely new work rather than wiring.

**M4.7 — unchanged.** The Replit scenario, through the real UI, by a real person. **The milestone gate.**

### The design stream (parallel, per `docs/plans/M4-visual-design.md`)

Tokens → typography → adopt Lucide → component kit + review route → motion spec. Then the mascot model sheet — **one character, four poses, one sign** (D18) — briefed from a cheap generated exploration pass.

This runs alongside the plumbing because it is independent, and because the interface should be built on the system rather than restyled afterwards.

---

### Sequencing rationale

- **Protocol before UI**, because the interface has nothing to display without it.
- **Tokens before the card**, so the card is built on the system instead of retrofitted.
- **Acceptance test last**, because it is the gate — and because it should be run against the finished thing, not a sketch.

---

## After M4

| Milestone | What it is | Exit |
|---|---|---|
| **M5** Onboarding | OpenRouter OAuth PKCE, two-click setup, spend cap, free-model mode | A non-technical tester reaches a first reply unaided |
| **M6** Packaging | MSIX, bundled Node runtime, clean install **and** uninstall | Works on a fresh Windows profile |
| **M7** Signing + distribution | Store submission (account is free), auto-update, winget | Installs with no SmartScreen warning |
| **M8** Hardening + audit | Threat model, injection test suite, published security posture | Published audit, no open criticals |

**Carried into M6 from the spikes, so they are not forgotten:**
- Re-take the **`app dir writable`** reading against a genuine `WindowsApps` install — the spike-002 reading came from a loose layout and is not representative.
- Answer the **MSIX state-path** question (D15): does a user-visible path survive virtualisation, or only the package-private store?
- The Gateway's Windows auto-start must become an **MSIX Start-up Task** — MSIX has no Scheduled Tasks.

---

## Critical path

**M4 → M5 → M6 → M7.** M8 overlaps. The design stream runs in parallel throughout and blocks nothing.

---

## What this needs from Jeff

**Nothing blocks M4.** The plumbing and the design system can proceed.

Later, and in order:
1. **Mascot commission** — quote-ready once the exploration pass picks a direction.
2. **OpenRouter credit** — for end-to-end testing at M5.
3. **A Store developer account** — free, needed at M7.
4. **Brazilian copy review** — before 1.0.

The Gateway restart queue is unrelated to the app but still pending.

---

## Risks

| Risk | Severity | Mitigation |
|---|---|---|
| **The protocol is designed twice** — the expensive kind of rework | **High** | Version it once, test round-trips, treat it as an interface not an implementation detail |
| **The wait can be bypassed** — would void the central promise | **High** | The sidecar blocks; a test actively attempts a bypass |
| **Scope creep into a chat app** | Medium | M4 is the approval layer, not a conversation UI |
| **Grant persistence collides with MSIX virtualisation** | Medium | D15 is answered at M6; keep the state path behind one function so it can move |
| Rust/webview build times stall the design loop | Low | A review route that reloads without a full rebuild |

---

## The one-paragraph version

**M4 is where the gate stops being a library and starts being a product.** The real work is unglamorous: the shell currently throws away the sidecar's output and speaks a one-command language, so the first job is a real bidirectional protocol and running the policy layer inside the sidecar. Then the approval round trip — where the wait must be genuine and unbrypassable — then the card, the herd, durable grants, and finally the Replit scenario run through a real interface by a real person. The design system runs alongside it so the interface is built on tokens rather than restyled later. After that it is onboarding, packaging, signing, and audit.
