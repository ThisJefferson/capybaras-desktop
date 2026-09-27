# Build Plan — "OpenClaw Desktop": a one-install agent that cautions before it acts

**Objective:** turn this agent into a signed, double-clickable Windows application that a non-technical person can install and use safely, where the agent **asks for confirmation before every consequential action.**

Compiled 2026-09-27. Companion to `research/safe-agent-blueprint.md`.

---

## 0. Definition of done

A person who has never opened a terminal can:

1. Download one file and install it with a double-click
2. Answer three plain-language questions during setup
3. Chat with the agent
4. **Be asked, in plain English, before the agent does anything that matters**
5. Undo anything the agent broke
6. Never see a YAML file, a JSON config, a CLI, or the word "sandbox"

If any of those six fails, it is not a consumer product — it is a developer tool with an installer.

---

## 1. What we are actually packaging

Facts gathered from the installed distribution, because they constrain everything downstream:

| Property | Value | Consequence |
|---|---|---|
| Package | `openclaw` v2026.9.4 | npm-distributed today |
| Entry | `bin: openclaw.mjs`, `main: dist/index.js` | Node CLI + long-running Gateway |
| Engine | `node >=24.16.0 <25 \|\| >=26.1.0` | We must ship a Node runtime; we cannot rely on the user's |
| Direct dependencies | **65** | Naive bundling will not work cleanly |
| Runtime model | Gateway (WebSocket server) + CLI + Control UI + plugins + skills | **A multi-process system, not a single binary** |
| Node SEA support | `--experimental-sea-config` available on 24.16.0 | Usable, but see §2 |
| Existing UI | A Control UI build already exists in the distribution | **Reuse it — do not rebuild** |
| Existing service | Gateway installs as a Windows Scheduled Task | The app should own this lifecycle instead |
| Existing policy hooks | `scheduledToolPolicy`, per-agent tool allow/deny | **The confirmation layer extends this — it is not new architecture** |

**The single most important takeaway:** OpenClaw is not a CLI that can be zipped into an `.exe`. It is a **local server plus a UI**, with a plugin ecosystem that installs from npm at runtime. The packaging must respect that.

---

## 2. Packaging architecture — three options

### Option A — Node SEA single executable

Bundle the CLI into one `.exe` with `node --experimental-sea-config`.

- **Pros:** one file; no Node install; official and stable since Node 22, improved in Node 24.
- **Cons:** SEA handles **static `require` graphs** well, and this project has 65 direct deps with dynamic loading, plus a plugin system that installs npm packages to disk at runtime. Native modules need manual asset handling. **And an `.exe` gives no UI** — which is the actual requirement.
- **Verdict: not the deliverable.** Possibly useful later as a headless "server build."

### Option B — Electron shell

Bundle Chromium + Node + the app.

- **Pros:** most mature path; the team already writes JavaScript; consistent rendering; mature auto-update via `electron-updater`; largest ecosystem.
- **Cons:** **150 MB+ installers** for a hello world; bundles an entire browser (a large additional attack surface, in a product whose selling point is safety); heavier RAM.
- **Verdict: the safe engineering default, and the wrong product choice here.** Shipping a security product with a browser engine bolted on is a bad look and a real liability.

### Option C — Tauri shell with a bundled Node sidecar ✅ **RECOMMENDED**

A Tauri (Rust) desktop shell that hosts the existing Control UI in the **OS WebView (WebView2)**, with a bundled portable Node runtime running the Gateway as a **sidecar child process**.

- **Pros:**
  - **Under ~10 MB** versus 150 MB+ — matters for download conversion
  - **Secure by default**, with per-API explicit enable/disable and a narrow IPC surface — the correct posture for a safety product
  - Uses WebView2, which is already present on Windows 11
  - The Control UI already exists and is web-based — it drops straight in
  - Tray icon for live status (our "bear")
  - Clean installer story (MSIX / MSI)
- **Cons:** introduces Rust into the toolchain; two runtimes to ship; sidecar lifecycle management is real work.
- **Verdict: recommended.** The size and the security posture are the product.

### Installer

- **MSIX** for the Microsoft Store path (§5) — handles signing, updates, and trust
- **MSI (WiX) or Inno Setup** for direct download and winget, if we self-distribute
- Ship the Node runtime inside the package; never require the user to install Node

---

## 3. The caution layer — the core requirement

This is the part Jeff actually asked for, and it is the product's reason to exist. It is not a dialog box. It is a **risk classification and consent system**.

### 3.1 Risk tiers

Every tool call gets classified before execution. Four tiers:

| Tier | Behaviour | Examples |
|---|---|---|
| **T0 — silent** | Runs, logged only | Reading a file in the workspace, fetching a page, searching |
| **T1 — notify** | Runs, then reports | Creating a new file, writing to a scratch area |
| **T2 — confirm** | **Pauses and asks** | Deleting, editing an existing document, sending a message, installing something |
| **T3 — hard gate** | **Requires typed confirmation + reason** | Mass deletion, spending money, sending to an external party, changing security settings, anything irreversible, anything touching credentials |

Classification inputs: does it mutate? is it reversible? how many objects? does it leave the machine? does it involve money or identity? is the input trusted?

**Critical rule:** classification is done by the **system**, not the model. A model that has been injected must not be able to argue its way down a tier.

### 3.2 The confirmation itself — eight rules

1. **Plain language, never tool names.** "Delete 1,240 customer records?" — not `db.delete({...})`.
2. **Show the blast radius.** How many, what kind, where.
3. **Show reversibility explicitly.** "Undo available for 30 days" or "**This cannot be undone.**"
4. **The safe option is the default.** Enter and the default button must be *Cancel*. Destructive choices are never the path of least resistance.
5. **Explain *why*.** One line of the agent's reasoning, so the user can spot a bad premise.
6. **Batch to prevent fatigue.** Five file writes in one task = one approval, not five. Fatigue is how gates die.
7. **Confirmation must arrive out-of-band from the untrusted input.** If the agent read a web page that told it to act, the approval prompt must not render in a context that page can influence. This is the single most important rule here.
8. **Timeout means no.** No answer on a T3 action = do not proceed.

### 3.3 Learn, but narrowly

The only sustainable answer to approval fatigue is memory — with hard limits:

- "Always allow" is offered **only for T2**, never T3
- Scoped to a specific action *and* a specific target (this folder, not "all folders")
- **Expires** (default 30 days)
- Visible in a plain-language list, revocable in one click
- Never grantable by the model — only by the human, through the GUI

### 3.4 Dry-run mode

A global toggle: *"Show me what you'd do before you do it."* The agent produces the full action plan, the user reviews it, then approves. For new users, **default this to ON for the first week.** It is the single most effective trust-building feature available, and it costs almost nothing to build.

### 3.5 The receipt

After every action, one plain-language line: *"Read 3 files from Documents. Sent 1 email to Sarah. Spent $0. Nothing else was touched."* One line, not a JSON trace.

### 3.6 Implementation path

This maps onto machinery that already exists — per-agent tool allow/deny lists, `scheduledToolPolicy`, and the approval-gate mechanism observed in this session. The work is:

1. A **risk classifier** in front of the tool dispatcher
2. A **GUI approval channel** in the Tauri shell
3. The **grant store** (scoped, expiring, human-only)
4. The **undo journal**

---

## 4. Phase plan

| Phase | Scope | Effort | Exit criteria |
|---|---|---|---|
| **0. Decisions** | Pick shell (Tauri), installer (MSIX + MSI), signing path, telemetry stance, target user | 1–2 wks | Signed-off spec |
| **1. Shell + sidecar** | Tauri app, bundled Node, Gateway as child process, tray status, Control UI hosted | 6–10 wks | Double-click launches a working agent on a clean VM |
| **2. Setup wizard** | Three questions, no config exposure, key entry via masked fields, model defaults pre-chosen | 3–4 wks | A non-technical tester completes setup unaided |
| **3. Caution layer** | Risk tiers, GUI approvals, grant store, dry-run, receipts | 4–6 wks | **No T2/T3 action can occur without a human approval** |
| **4. Containment** | Workspace-only files, domain allowlist, ephemeral per-task sandbox, credential broker | 8–12 wks | Secrets never readable by the agent process |
| **5. Undo** | Snapshot-before-mutate, retention window, one-click restore | 4–6 wks | A deleted file is restored from the UI in < 30 s |
| **6. Distribution** | Signing, MSIX, Store listing, winget manifest, updater | 3–5 wks + reputation wait | Install on a fresh machine with no warning |
| **7. Hardening** | Third-party audit, threat model, injection test suite | 4–6 wks | Published audit; no open criticals |

**Rough total: 6–9 months for a small team.** MVP (§9) is the first ~3 months.

### Sequencing note

Phase 3 (**the caution layer**) is deliberately before Phase 4 (**the sandbox**). Reason: the sandbox is harder and takes longer, and the confirmation gates deliver most of the user-visible safety benefit for a fraction of the effort. Ship the gates first.

---

## 5. Distribution and signing — the honest blocker

This is where "anyone can use it" actually gets decided, and the news is worse than most developers expect.

**What changed:** As of **March 2024, EV code-signing certificates no longer bypass SmartScreen.** Microsoft's own documentation now states the previous behaviour "no longer exists," and EV-signed files "go through the same reputation-building process as OV certificates." A brand-new, properly signed binary will still show *"Windows protected your PC"* until reputation accrues. Anyone selling you an EV certificate on the promise of instant trust is describing a behaviour that ended two years ago.

**The options, honestly compared:**

| Path | Cost | SmartScreen | Notes |
|---|---|---|---|
| **Microsoft Store (MSIX)** | $0 — free since Sept 2025 (individuals) and May 2026 (companies) | **Handled** — Store apps are trusted | **Cleanest consumer path.** Store handles signing and updates. Some sandboxing constraints (see below) |
| **Azure Artifact Signing** (formerly Trusted Signing) | **$9.99/mo** (5k signatures) or $99.99/mo (100k) | Reputation still builds — *not* instant | No hardware token; CI/CD friendly. Availability: organizations US/CA/EU/UK; **individuals US/CA only** |
| **OV certificate** | ~$200–400/yr + token | Reputation builds | Functionally equivalent to Artifact Signing for SmartScreen |
| **Unsigned** | free | Bad warning, poor conversion | Not acceptable for this product |

**Recommendation: publish to the Microsoft Store as primary, and also ship a signed MSI for winget and direct download.** The Store removes the reputation problem entirely and gives automatic updates. Keep the MSI path because power users and IT departments prefer it, and winget manifests accept MSI/MSIX/EXE installers (script-based installers are not accepted).

**One Store caveat to design around:** Store packaging constrains some system-level behaviours. Given this app runs a local server and writes to a user data directory, verify early that the Gateway's file and network patterns are Store-compatible. **Do this in Phase 0, not Phase 6** — discovering it late would force a repackage.

**Partly resolved, and one part is new.** Research (`docs/research/packaging-and-signing.md`) confirms MSIX *full-trust* packages should permit Node sidecar spawning — but no single source documents MSIX + Tauri sidecar + Store submission together, so the Phase 2 spike must test it specifically before packaging work begins.

**And a genuine architecture change:** MSIX packages do not support traditional Scheduled Tasks. The Gateway's Windows auto-start must therefore become an **MSIX Start-up Task** under Store distribution. See `DECISIONS.md` D11.

---

## 6. Updates

Non-negotiable: **the updater is a security control.** An agent with shell access and no update path is a permanent vulnerability.

- Store builds: handled by the Store
- MSI builds: signed update manifest, verified signature before install, no silent installs of unsigned payloads
- **Always offer "what changed" in plain language** — this product's users will not read a changelog, but they will read one sentence

---

## 7. Security requirements (non-negotiable list)

If any of these slip, the product's core promise is broken:

1. **No plaintext secrets on disk.** Credential broker only; the agent never reads a key. *(We found twenty plaintext secrets on this machine this morning — that is the default state of an expert install, and it is exactly what this product exists to fix.)*
2. **Files scoped to a workspace** by default, widened only by explicit user action
3. **Network off by default** for the agent; domain allowlist when enabled
4. **Every mutating action journalled** and reversible
5. **Approval channel isolated from untrusted input**
6. **Signed releases only**, verified before execution
7. **No silent capability escalation** — new permissions are always a human decision
8. **Third-party audit before 1.0**

---

## 8. Risks and open questions

| Risk | Severity | Mitigation |
|---|---|---|
| **Approval fatigue** — users click through everything within a week | **High** | Fewer, better gates; T2-only "always allow"; batching; dry-run for the first week |
| Store packaging blocks the Gateway's process/file model | **High** | Verify in Phase 0; keep MSI path as fallback |
| Sidecar lifecycle bugs (orphaned Node processes, port conflicts) | Medium | Process supervision, single-instance lock, health checks |
| Containment breaks legitimate workflows | Medium | Already proven this morning; make widening one click, and never silently fail |
| Reputation wait delays adoption | Medium | Store-first; MSI as parallel path |
| Model cost surprises the user | Medium | Show spend in the receipt; hard monthly cap on by default |
| An audit finds something structural late | Medium | Threat model in Phase 0; audit at Phase 7, not after launch |

**Open questions requiring Jeff's input:**
1. Target user — the prosumer, or genuinely non-technical?
2. Local models only, or cloud API keys? (Changes the setup wizard and the privacy story substantially.)
3. Who is the publisher of record for signing and the Store account?
4. Free/paid/open-core?

---

## 9. MVP — the three-month version

Everything above is 6–9 months. The shippable-early version:

- Tauri shell, bundled Node, working Gateway, tray status
- Three-question setup wizard
- **Risk tiers + T0/T1/T2 gates** (T3 hard gates can follow)
- Workspace-only files, network off by default
- Dry-run toggle **on** for the first week
- Receipts after every action
- Signed **MSI**, distributed direct + winget (Store submission in parallel)
- No sandbox, no credential broker, no undo yet — **but the promise already holds**, because the agent asks before it acts and confines itself to a workspace

That MVP is genuinely safer than the status quo for a normal user, and it is honest about what it does not yet do.

---

## 10. The one-sentence version

**Package what already exists behind a Tauri shell, put a risk classifier in front of every tool call, ask in plain English before anything that matters, sign it, and ship it through the Store — and say out loud in the marketing that it will still break, just small and undoably.**

---

## Sources

**Packaging**
- Node.js Single Executable Applications — stable since Node 22, improved in Node 24 (code cache, assets API); `--experimental-sea-config` verified present on Node 24.16.0 on this host
- Vercel `pkg` — legacy; migration to SEA advised
- Tauri vs Electron — WebView2-based Tauri at <10 MB vs Electron at 150 MB+; Tauri secure-by-default with explicit API gating
- Local distribution facts: `openclaw` v2026.9.4, `bin: openclaw.mjs`, engines Node ≥24.16.0, 65 direct dependencies

**Signing and distribution**
- Microsoft Learn, *SmartScreen reputation for Windows app developers* — EV certificates no longer bypass SmartScreen; Artifact Signing recommended for non-Store distribution
- Microsoft Learn, *Code signing options for Windows app developers* — "Expect SmartScreen prompts for new files until reputation is established"
- Azure Artifact Signing (formerly Trusted Signing) — $9.99/mo Basic (5,000 signatures), $99.99/mo Premium (100,000); availability limited by region
- Microsoft Learn, *Publish your first Windows app* — winget accepts MSIX/MSI/APPX/EXE installers; script-based installers are not supported
- Microsoft `winget-pkgs` — YAML manifests, community submission; `wingetcreate` tooling

**Prior work**
- `research/safe-agent-blueprint.md` — architecture and UX model
- `research/agentic-harm-and-the-next-ai-winter.md` — the harm classes these gates address
- This session, 2026-09-27 — live audit and hardening pass used as the worked example

---

*Method note: packaging, signing, and pricing figures are drawn from Microsoft's own current documentation and were verified against the installed distribution on this machine. The effort estimates are engineering judgements with wide error bars, not measurements. §5 is the section most likely to change, because Microsoft's signing policy has moved twice in two years.*

---

# PART II — Decisions locked (2026-09-27)

Jeff answered all four open questions:

| Question | Decision |
|---|---|
| Target user | **Genuinely non-technical** |
| Models | **Cloud API keys**, with easy credit top-up and key creation |
| Licence | **Free**, donations + GitHub recognition |
| UI | **Simple download and install, buttery smooth** |

Each answer changes the design. Two of them change it a lot, and one removed the single hardest problem in the plan.

---

## 11. Licensing — checked, and the news is good (with one trap)

**OpenClaw is MIT licensed.** Verified in the installed distribution:

> MIT License, Copyright (c) 2026 OpenClaw Foundation
> "...to deal in the Software without restriction, including without limitation the rights to **use, copy, modify, merge, publish, distribute, sublicense, and/or sell** copies of the Software..."

**So redistribution inside a bundled free product is explicitly permitted.** This is the best possible answer — permissive, no copyleft obligations, no source-disclosure requirement for our own additions.

**The one obligation:** the copyright notice and permission notice must be included in all copies or substantial portions. Practically:
- Ship OpenClaw's `LICENSE` inside the installer
- Ship `THIRD_PARTY_NOTICES.md` (the package explicitly records incorporated/adapted third-party code there — this is a real, existing file and it must travel with the build)
- Surface both in an **About → Open source licences** screen
- Add a CI licence-audit step that regenerates notices on every dependency change

### The trap: MIT covers copyright, not trademarks

**The MIT licence does not grant any right to the OpenClaw name or marks.** Naming the product "OpenClaw Desktop" and shipping it under the Foundation's brand invites a trademark complaint even though the code is free to use.

**Recommendation:**
1. **Pick a distinct product name** (the TunnelBear-inspired brief suggests something friendly and mascot-shaped, e.g. a creature name — a name that can also carry the status character)
2. Describe it factually: *"a desktop app built on OpenClaw"* — with attribution and a link
3. Optionally write to the OpenClaw Foundation and ask for a blessing — permissive projects frequently grant one, and it costs nothing to ask

### Our own licence

Go **Apache-2.0** rather than MIT. For a security product that people are being asked to trust with file access, the **explicit patent grant** matters, and it is the licence serious evaluators prefer. It is equally donation-friendly and equally permissive.

**And note what MIT already does for us:** it disclaims all warranty. For a free product that can delete files, that matters.

---

## 12. The onboarding breakthrough — the hard problem is already solved

In Part I (§3, §8) the single biggest risk to this product was stated as: **a non-technical user cannot reasonably obtain an API key.** It involves creating an account, verifying email, adding a card, finding a developer console, generating a key, and pasting a 64-character string. Every one of those steps is a dropout point.

**That problem is solved, and we do not have to build it.**

**OpenRouter supports OAuth PKCE that issues an API key.** The flow: the user clicks one button, approves in their browser, and a user-owned key is created and stored automatically. In the words of a 2026 implementation note: *"Users approve in the browser and a user-owned API key is created and saved for them, so there's nothing to copy or paste."*

**And OpenClaw already supports it.** From the official provider docs: *"OpenRouter OAuth is a PKCE login flow that issues an OpenRouter API key, so OpenClaw stores the result in the same `openrouter:default` API-key auth profile used by manual API-key setup."*

That means the flow is **configuration we wire up, not engineering we invent.**

**OpenRouter also exposes key provisioning endpoints** — create, read, update, delete API keys programmatically. That is what powers "create a new key" and "revoke this key" inside the app, with no console visit.

### Why OpenRouter is the right single default

- **One account, one balance, many models** — no provider menu to present to a non-technical user
- One place to add credit
- Free models exist, so a user can try the product before paying anything
- It is where the developer already is, and it is a well-known industry default

**Do not present a provider menu on first run.** Offer OpenRouter as *the* path, and hide "use a different provider / paste a key" behind an Advanced door for power users.

### Revised setup wizard — two real decisions

| Screen | Content | User effort |
|---|---|---|
| **1. Welcome** | What this is, what it can do, and one honest line about what it can't | Read for 20 seconds |
| **2. Connect** | **One button: "Sign in with OpenRouter."** Opens browser, user approves, key arrives automatically | One click |
| **3. Credit** | Shows balance. If zero: **"Add credit →"** deep link to the top-up page, plus a "try the free models first" option | One click |
| **4. Done** | — | — |

### Residual friction, stated honestly

The user still needs an OpenRouter **account and a payment method** at some point. That cannot be designed away. What can be done:
- An annotated walkthrough with real screenshots for the account-creation step
- A **free-model mode** that works with zero credit — so the first experience never hits a paywall
- Clear, plain-language messaging: *"This app is free. You pay OpenRouter directly for what the AI uses — usually a few dollars a month."*
- **A default monthly spend cap, on, set low** — the single most important trust feature in the whole product, because it makes the worst case a boring number

### Built-in money awareness

Non-technical users fear runaway bills more than breaches. So:
- Spend shown in the status area, always visible
- A plain-language receipt: *"This month: $3.41. Cap: $10."*
- Warn at 70% of cap, in the user's own words not a percentage
- Nothing that costs money ever happens without a T2/T3 gate (§3)

---

## 13. Free, donations, and GitHub recognition

### The economics actually work

| Cost | Amount |
|---|---|
| GitHub hosting, releases, downloads | **$0** |
| Microsoft Store developer account | **~$19 one-time** |
| Azure Artifact Signing (if used) | **$9.99/mo (~$120/yr)** |
| **Inference** | **$0 — the user's own key pays for it** |
| Domain, if desired | ~$12/yr |

**Total cash: roughly $20–150 per year.** That is sustainable on donations.

**And BYOK is what makes "free" possible at all.** If we paid for inference, a free product would be a subsidy with no ceiling. Because each user brings their own account, the maintainer's cost is fixed and tiny. **Say this out loud in the README** — it is simultaneously the business model and the privacy story.

### Donations

- **GitHub Sponsors** as primary — native, appears as a Sponsor button on the repo, zero friction
- Add `.github/FUNDING.yml` so the button actually appears
- Secondary: Ko-fi or Open Collective for people who prefer not to use GitHub billing
- **Never gate features.** Donations only. Any paywall in a safety product undermines the trust argument that is the entire product.

### The recognition playbook

What actually drives a GitHub project from zero:

1. **A demo GIF above the fold.** This is the single highest-impact asset. Show the agent asking permission and the user approving — that is the differentiator, so show it first.
2. **A 45–60 second video.** Silent, captioned, no talking head.
3. **One-line install** at the top: a download button, not a build guide.
4. **A "What it does / What it does NOT do" table.** The honest-limits section reads as integrity, not weakness, and it is the reason a security-minded visitor will trust the download.
5. **A comparison table** against the obvious alternatives.
6. `SECURITY.md` with a disclosure policy — non-optional for this product category
7. `CONTRIBUTING.md`, issue templates, Discussions enabled, and a handful of genuine `good first issue`s
8. **Plain-language changelogs** with every release
9. Semantic versioning, and signed releases from day one

**Positioning line to build everything around:** *the agent that asks first.*

---

## 14. The UI spec — what "buttery smooth" means concretely

"Buttery smooth" is a measurable engineering target, not a vibe. The budget:

### Performance
- **60 fps** for all animation, no exceptions
- **Visible response within 100 ms** of every user action — always, even if the work takes longer
- **First meaningful paint under 400 ms** cold start
- **Zero layout shift**
- Agent work runs in a separate process, so the UI thread is never blocked — architecture already gives us this

### Interaction
- **Streaming text, never a spinner.** Tokens appear as they arrive; the interface feels instant even when it isn't
- **Skeletons, not spinners**, for anything that loads
- **Optimistic UI for approvals** — the button responds immediately, the result resolves behind it
- **Animation: 150–250 ms, ease-out.** Spring physics on the status character only. Never animate on scroll
- **Respect `prefers-reduced-motion`** — required, not optional

### The four states of the status companion

This is the TunnelBear move, made functional:

| State | Meaning | Motion |
|---|---|---|
| **Idle** | Nothing happening | Slow breathing |
| **Listening** | Waiting for you | Subtle attentive tilt |
| **Working** | Doing something | Purposeful activity |
| **Needs you** | **An approval is waiting** | **The most visually distinct state in the app** |

"Needs you" must be unmistakable from across the room. It is the state that carries the entire product promise.

### The hero component

The **approval card** is the most important thing in the interface, and it should be designed first, before any chat UI:

- Plain-language sentence, no jargon
- Blast radius in numbers
- Reversibility stated explicitly
- **Cancel is the default button; Enter must never approve something destructive**
- One-line rationale from the agent
- Big, legible, unhurried

### Visual language
- System font stack (Segoe UI Variable on Windows 11), **16 px minimum body text**
- A 4/8/12/16/24/32 spacing scale and a real type scale — tokens, not ad-hoc values
- Three accent colours maximum; one of them reserved for "needs you"
- WCAG AA contrast minimum, full keyboard navigation, visible focus rings
- **Explicitly avoid:** enterprise grey, dense tables, tiny type, jargon, nested settings trees

### Vocabulary discipline

Words that must never appear in the interface: *gateway, sandbox, MCP, plugin, skill, token, API, config, JSON, YAML, loopback, provider*.

If a screen needs one of those words to make sense, the screen is wrong.

---

## 15. What changed in the plan

| Item | Before | Now |
|---|---|---|
| Setup wizard | 3 questions, key pasting | **2 clicks**, no key handling |
| Biggest risk | Non-technical users can't get an API key | **Retired** — OpenRouter OAuth, already supported |
| Biggest remaining risk | — | **Approval fatigue** (§3) |
| Signing cost | Unknown | $19 one-time (Store) or ~$120/yr (signing) |
| Licence | Unverified | **MIT — clear to redistribute.** Trademark caution on naming |
| Product name | "OpenClaw Desktop" | **Must change** — MIT does not grant trademark rights |
| UI brief | "TunnelBear-like" | Measurable spec (§14) |

**The critical path is unchanged and it is not the packaging.** It is the **approval layer** (§3) — because that is what makes the product trustworthy, and because fatigue is what kills approval layers.

---

## Sources (Part II)

- OpenClaw `LICENSE` (MIT, Copyright 2026 OpenClaw Foundation) and `THIRD_PARTY_NOTICES.md`, read from the installed distribution on this machine
- `package.json` — `license: MIT`, repository `github.com/openclaw/openclaw`, author OpenClaw Foundation
- OpenRouter docs — *OAuth PKCE* (issues an API key via a browser approval flow) and *Provisioning API Keys* (programmatic create/read/update/delete)
- OpenClaw docs — *OpenRouter* provider page: OAuth PKCE issues a key stored in the `openrouter:default` auth profile
- Microsoft Learn — Store developer account, code signing options, SmartScreen reputation (see Part I §5)

*Method note: the licence text and the OpenRouter/OpenClaw OAuth support were verified directly rather than assumed. The trademark caution is a practical risk judgement, not legal advice — if the project gains traction, a five-minute conversation with the Foundation is worth more than this paragraph.*
