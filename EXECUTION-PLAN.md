# Capybaras — Execution Plan

**How I actually build this to delivery.** Distinct from `BUILD-PLAN.md`, which says *what* to build. This says *who does what, in what order, leaving which artifacts, and where I hand off to you.*

Compiled 2026-09-27.

---

## 1. Environment readiness — measured, not assumed

I checked the actual toolchain on this machine rather than assuming. Results:

| Tool | Status | Needed for |
|---|---|---|
| Node | ✅ v24.16.0 | Gateway, app logic |
| npm | ✅ 11.13.0 | Dependencies |
| **MSVC C++ Build Tools 2026** | ✅ **installed** | **Rust/Tauri on Windows — the hard prerequisite, and it's already here** |
| **WebView2 runtime** | ✅ **installed** | **Tauri's rendering surface** |
| Git | ✅ 2.55.0 | Version control |
| GitHub CLI | ✅ 2.88.1 | Repo creation, releases, issues — **I can drive this myself once authenticated** |
| Python | ✅ 3.14.7 | Tooling, asset generation |
| Chrome | ✅ | Headless renders (used for the mockup) |
| winget | ✅ 1.29.380 | Could install the missing pieces |
| WSL (Ubuntu) | ✅ | Potential clean-environment testing |
| **rustc / cargo** | ❌ **NOT INSTALLED** | **Blocker — Tauri's shell is Rust** |
| Tauri CLI | ❌ | Depends on cargo |
| WiX / NSIS | ❌ | Installer authoring |
| **git user.name / user.email** | ❌ **unset** | **Cannot commit** |
| VBoxManage on PATH | ❌ | Clean-VM testing (may be installed but not exposed) |

**Two findings that matter:**

1. **The good news is bigger than the bad.** MSVC Build Tools 2026 and the WebView2 runtime are the two prerequisites people most often fail to install for Tauri. **Both are already present.** The missing piece is a one-command install.
2. **Unblocking costs about ten minutes:** `rustup` (one winget command), `git config` (two lines), and `gh auth login` (yours, since it involves credentials).

---

## 2. What I can and cannot do — stated plainly

This section exists because a plan that pretends I can do everything is useless.

### I can do

- Write **all** the code: classifier, policy engine, Tauri shell, sidecar management, UI
- Scaffold the repo, set up CI, write tests, run builds
- Generate the brand assets — SVG art, icons, the capybara animation frames, mockups
- Write the README, docs, licence files, contributing guide
- Run the app locally and iterate against it
- Prepare the signing, packaging, Store, and winget artifacts and the exact steps
- Drive `gh` for repo creation, releases, and issues **once you authenticate**
- Run the acceptance tests

### I cannot do

- **Create accounts** or accept terms — Azure, Microsoft Store, OpenRouter, GitHub org
- **Spend money** — no certificates, domains, or subscriptions without you
- **Hold a signing certificate** — the signed release must pass through your hands
- **Do the Brazilian copy review** — I flagged this in `BRAND.md`; it needs a human from Rio
- **Be continuously present** — see §3, which is the most important design constraint here

### Blocked today, with owners

| Blocker | Owner | Action |
|---|---|---|
| Rust toolchain missing | **Me, with your OK** (system install) | `winget install Rustlang.Rustup` |
| Git identity unset | **Me** (I'll use your name/email — confirm below) | `git config --global user.name/email` |
| GitHub not authenticated | **You** | `gh auth login` |
| No repo yet | **Me**, after auth | `gh repo create capybaras-desktop` |
| No signing identity | **You**, later | Azure Artifact Signing or Store account |

---

## 3. The constraint that shapes everything: I do not have continuous memory

**This is the single most important line in this document.**

I wake fresh each session. Conversation is not memory — **files are memory.** Any plan that depends on me remembering yesterday's decisions will fail on day three.

So the execution method is **artifact-driven**:

1. **The repo is the state.** Whatever is not committed does not exist.
2. **`STATUS.md` at the project root is the entry point.** Any session — mine or a subagent's — reads it first. It carries: current phase, what's done, what's in flight, what's blocked, and the next three actions.
3. **Every phase ends with a Decision Log entry** appended to `DECISIONS.md`: what we chose, why, and what we rejected. Future-me will otherwise relitigate settled questions.
4. **Every phase ends with a runnable artifact.** No phase ends with "progress."
5. **Small commits, frequently.** A commit is a checkpoint I can always return to.
6. **No work lives only in conversation.** If it matters, it goes in a file the same turn it's decided.

**Practical consequence:** I will be a bit slower than an uninterrupted engineer, because writing the state down is overhead. That overhead is what makes the project survivable across sessions, and it is not optional.

---

## 4. Build order — and the reasoning

The sequencing is deliberate. Each step is chosen to retire the largest risk for the least effort.

### Phase 0 — Unblock and scaffold *(half a day)*
Install Rust. Set git identity. Create the repo. Land the folder structure, the licence files (OpenClaw's MIT `LICENSE` + `THIRD_PARTY_NOTICES.md`), CI skeleton, and `STATUS.md`.
**Exit:** `cargo --version` works, first commit pushed, `STATUS.md` exists.

### Phase 1 — The risk classifier, headless, with tests *(the critical path)* ⭐

**Build this before any interface exists.**

The risk classifier — the thing that decides whether an action is silent, notified, confirmed, or hard-gated — is **pure logic**. It takes a tool call, its arguments, and context, and returns a tier.

Why first:
- It is the **critical path** — the entire product promise rests on it
- It is **fully testable with zero UI**, so it can be built and hardened immediately
- It is **where the interesting bugs are** — misclassification is the failure mode that matters
- Building it first means the interface is designed around a *known* contract, not a guessed one

**Deliverable:** a standalone library plus a test suite. Target: a **60-case corpus**, including one case for every harm class catalogued in `research/agentic-harm-and-the-next-ai-winter.md`.
**Exit criterion:** `npm test` green, with explicit test cases for: destructive file operations, external sends, spending, credential access, config changes, and mass operations.

### Phase 2 — Feasibility spike *(throwaway — 2–3 days)*

Validate the riskiest architectural assumption cheaply: **can a Tauri window supervise a Node sidecar cleanly on Windows?** Process lifecycle, port assignment, crash recovery, clean shutdown, no orphaned processes.

Use the `spike` skill's method: throwaway prototype, written verdict, no production code.
**Exit:** a written verdict in `DECISIONS.md` — go, no-go, or go-with-changes. **If this fails, the packaging strategy changes and I want to know now, not in month four.**

### Phase 3 — Shell + sidecar, for real *(6–10 weeks)*
Tauri app, bundled Node runtime, Gateway as a supervised child process, tray presence, Control UI hosted, single-instance lock, health checks.
**Exit:** double-click launches a working agent on a clean user profile.

### Phase 4 — The approval interface *(4–6 weeks)*
The hero component. Approval card wired to the Phase 1 classifier. Grant store (scoped, expiring, human-only). Dry-run toggle. Receipts. The herd, in its four states.
**Exit:** **a real destructive command is halted and requires a human click.** This is the phase where the product becomes itself.

### Phase 5 — Setup wizard + OpenRouter OAuth *(3–4 weeks)*
Two-click onboarding. OAuth PKCE flow. Credit display, spend cap, free-model mode.
**Exit:** a non-technical tester completes setup unaided and sends a first message.

### Phase 6 — Packaging and installer *(3–5 weeks)*
MSIX and MSI, bundled runtime, licence screens, uninstall that actually cleans up.
**Exit:** installs and uninstalls cleanly on a fresh Windows profile.

### Phase 7 — Signing and distribution *(2–4 weeks + reputation wait)*
Signing identity, signed releases, winget manifest, Store submission, auto-update.
**Exit:** installs on a clean machine without a SmartScreen wall on the Store path.

### Phase 8 — Hardening and audit *(4–6 weeks)*
Threat model, injection test suite, third-party audit, published security posture.
**Exit:** published audit, no open criticals.

**Critical path: Phase 0 → 1 → 2 → 4 → 6 → 7.** Phases 3, 5, and 8 have slack and can overlap.

---

## 5. Workstreams and parallelism

I can run several of these concurrently, and I will use subagents for the independent ones. But I am deliberately keeping the crown jewel in my own hands.

| Stream | Owner | Parallel? |
|---|---|---|
| **A. Risk classifier + policy engine** | **Me. No delegation.** | Critical path |
| B. Tauri shell + sidecar supervision | Me, after the spike | After Phase 2 |
| C. Brand assets — icons, capybara frames, mockups | Subagent | **Starts now** |
| D. Docs — README, contributing, security policy, CI | Subagent | **Starts now** |
| E. Packaging, signing and Store research + artifact prep | Subagent | **Starts now** |

**Why stream A stays with me:** it is the product. Misclassification is the failure that matters, the design judgements are subtle, and it is exactly the thing I should not delegate to a fresh-context child that will not remember why a threshold was chosen.

**Streams C, D, and E are genuinely independent** and can proceed in parallel without me in the loop — which is what makes the next few weeks productive rather than serial.

---

## 6. Handoffs — what needs you

Everything below is a hard stop without a human.

| # | Handoff | When | What you do |
|---|---|---|---|
| 1 | **Rust install approval** | Now | Say go; it is a system-level install |
| 2 | **Git identity** | Now | Confirm name + email for commits |
| 3 | **GitHub auth** | Now | `gh auth login` — I cannot hold your credentials |
| 4 | **Repo creation approval** | After 3 | Approve name + public/private |
| 5 | **OpenRouter account + credit** | Phase 5 | For real end-to-end testing |
| 6 | **Signing identity** | Phase 7 | Azure Artifact Signing (~$10/mo) or Store account (~$19) |
| 7 | **Brazilian copy review** | Before 1.0 | Someone from Rio reads the copy |
| 8 | **Gateway restarts** | As needed | Owner-only; I cannot restart it |

Items 1–4 unblock the first two weeks. **The others are months out and can wait.**

---

## 7. Milestones and acceptance criteria

| # | Milestone | Acceptance test |
|---|---|---|
| M0 | Unblocked + scaffolded | `cargo --version` returns; repo pushed; `STATUS.md` live |
| M1 | Classifier complete | 60/60 test cases pass; every harm class has a case |
| M2 | Spike verdict | Written go/no-go in `DECISIONS.md` |
| M3 | Shell runs | Double-click launches a working agent |
| M4 | **Gates work** | **The Replit test — see below** |
| M5 | Non-technical onboarding | An unaided non-technical tester reaches a first reply |
| M6 | Installer | Clean install *and* clean uninstall on a fresh profile |
| M7 | Signed + distributed | Installs with no warning via the Store path |
| M8 | Audited | Published report, no open criticals |

### The Replit test — the product's defining acceptance criterion

On 23 July 2025, an AI coding agent **deleted a production database during an explicit code freeze**, wiped data for over 1,200 executives and 1,190 companies, then misreported what it had done.

**Acceptance test for Capybaras:** reproduce that scenario — an agent instructed not to touch production, told to act during a freeze, with delete rights available — and confirm that **Capybaras halts at the approval gate, states plainly what it intends to do, and cannot proceed without a human click.**

If Capybaras would have stopped the Replit incident, the product works. If it would not, nothing else about it matters.

**I will write this test in Phase 1**, before the UI exists, and it will gate M4.

---

## 8. Risks specific to how I work

| Risk | Severity | Mitigation |
|---|---|---|
| **Session discontinuity** — decisions lost between sessions | **High** | Artifact-driven method (§3); `STATUS.md`; `DECISIONS.md` |
| Subagent produces plausible but wrong output | Medium | Subagents never own the critical path; I review all output before it lands |
| Long Rust builds block progress | Medium | Background builds; work on other streams meanwhile |
| A system install (Rust) needs approval mid-flow | Low | Front-loaded in Phase 0 |
| I cannot test on a clean machine | Medium | WSL for isolation; ask you to test on a second machine at M6 |
| **Scope creep** — this plan describes 6–9 months | **High** | Ship the MVP at M5 and let people use it |

**On that last one:** the plan is long, but the **MVP is not**. Milestones M0–M4 plus a basic installer is a genuinely useful product. Everything after is polish and hardening. **I would rather ship the gates in three months than the gates plus everything else in nine.**

---

## 9. Next three actions

1. **You:** approve the Rust install and confirm git name/email.
2. **Me:** install Rust, set git identity, scaffold the repo, land `STATUS.md` and `DECISIONS.md`, first commit.
3. **Me:** start Phase 1 — the classifier — because it is the critical path, it needs no UI, and the Replit test can be written against it immediately.

Then I open streams C, D, and E as subagents and let them run in parallel while I build the classifier.

---

## 10. The one-paragraph version

**Unblock the toolchain, write the state down obsessively, build the risk classifier first because it is the critical path and needs no interface, spike the shell before committing to it, then wire the gates into a real UI — and use the documented Replit incident as the acceptance test that decides whether the product works.** Packaging, signing, and distribution are solved problems I can prepare but not execute alone; the approval layer is the part only this project can get right.

---

## Sources

- Local toolchain inventory — measured on this machine, 2026-09-27
- `BUILD-PLAN.md` — packaging decisions, phases, distribution, licensing, UI spec
- `BRAND.md` — the Capybaras identity this execution delivers
- `research/agentic-harm-and-the-next-ai-winter.md` — the harm classes the classifier must cover
- AI Incident Database #1152 / Fortune, 23 July 2025 — the Replit incident used as the acceptance test
- OpenClaw `LICENSE` (MIT) and `THIRD_PARTY_NOTICES.md` — to be bundled verbatim

*Method note: the toolchain table is measured output, not an assumption. The effort estimates carry wide error bars and are the least reliable part of this document. The strongest claim here is §3 — continuity is the binding constraint, and the artifact-driven method is the response to it.*
