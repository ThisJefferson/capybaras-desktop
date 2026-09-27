# DECISIONS

> Why things are the way they are. **Read before relitigating.**
> Format: decision · date · rationale · what was rejected.

---

## D1 — Packaging: Tauri shell + bundled Node sidecar
**2026-09-27**

A Rust/Tauri desktop shell hosting the existing Control UI in WebView2, with a bundled portable Node runtime running the Gateway as a supervised child process.

**Rejected:**
- **Node SEA single executable** — handles static dependency graphs, but this project has 65 direct deps, dynamic loading, runtime npm plugin installs, and **no UI**. A single `.exe` is not the deliverable.
- **Electron** — most mature, but 150 MB+ and bundles an entire browser. Shipping a *security* product with a full browser engine bolted on is a liability and a bad look.

**Reasons for Tauri:** under ~10 MB, secure-by-default with explicit API gating, WebView2 already present on target machines, and the Control UI already exists as a web app.

---

## D2 — Name: **Capybaras** (always plural); repo slug `capybaras-desktop`
**2026-09-27**

**Checked:** `Capybara` (singular) is an established MIT Ruby acceptance-testing framework (`teamcapybara/capybara`), with its own Wikipedia page. Bare `capybara` is taken as a software name.

**Decision:** the plural is both distinct and *is the concept* (multiple capybaras = multiple agents). Avoid the bare slug for repo, npm package, or CLI binary. Use `capybaras-desktop`.

**Also:** the product name must not use the **OpenClaw** mark — MIT grants copyright rights, not trademark rights. Attribution is factual: *"a desktop app built on OpenClaw."*

---

## D3 — Our licence: Apache-2.0. Upstream: MIT, vendored separately
**2026-09-27**

- **Our code → Apache-2.0** (`./LICENSE`). Chosen over MIT for the explicit **patent grant**, which matters for software people are asked to trust with file access.
- **OpenClaw's MIT → `LICENSES/openclaw-MIT.txt`** (not the repo root — putting it at the root would misattribute the whole project to the Foundation).
- `THIRD_PARTY_NOTICES.md` travels with the build; must also ship inside the installer.

**Caught during scaffolding:** the first `git add` staged OpenClaw's MIT licence as the project root `LICENSE`. Fixed before the first commit.

---

## D4 — The project repo is nested inside the agent workspace
**2026-09-27**

The workspace is itself a git repo. The project lives at `workspace/projects/safe-agent-desktop/` as its own repo, and the path is added to the parent's `.git/info/exclude` so the parent never picks it up.

**Reason:** `tools.fs.workspaceOnly` is set to `true` in the gateway config (pending restart). Keeping the code inside the workspace means file tools keep working on it.

---

## D5 — Build the risk classifier **first**, headless, before any UI
**2026-09-27**

The critical-path decision. The classifier decides whether an action is silent, notified, confirmed, or hard-gated. It is pure logic, fully testable with zero interface, and it is where the interesting bugs live.

Building it first means the UI is designed around a **known contract** rather than a guessed one.

---

## D6 — Acceptance test: the Replit incident
**2026-09-27**

The product's defining test. Reproduce the 23 July 2025 incident — an agent with delete rights, instructed not to touch production, acting during a code freeze — and confirm **Capybaras halts at the approval gate and cannot proceed without a human click.**

Written in Phase 1, before the UI exists. Gates milestone M4.

---

## D7 — Onboarding: OpenRouter OAuth PKCE, not key pasting
**2026-09-27**

The biggest risk in the first draft of the plan was that a non-technical user cannot obtain an API key. **OpenRouter supports OAuth PKCE that issues a key via browser approval**, and OpenClaw already stores the result in its standard auth profile.

Setup becomes two clicks with no key handling. Provider menu hidden behind Advanced. A low default monthly spend cap is on from day one.

---

## D8 — CI workflow held back from the first push
**2026-09-27**

`.github/workflows/ci.yml` was rejected on push: the `gh` token carries `gist`, `read:org`, `repo` — **no `workflow` scope**.

**Decision:** keep the file on disk, exclude it from commits via `.git/info/exclude`, and push without it rather than block M0. Enable once the scope is granted: `gh auth refresh -s workflow`.

---

## D9 — Git identity uses a noreply address
**2026-09-27**

`264281275+ThisJefferson@users.noreply.github.com` — GitHub's ID-prefixed noreply format. Prevents the real email address from appearing in public commit history.

---

## D10 — Every hazard gets its own reason, even when the tier doesn't move
**2026-09-27**

The first implementation only recorded a reason when the tier actually rose. The Replit acceptance test caught it immediately: an action that was **simultaneously** off-limits, 1,200 rows, and irreversible reported exactly **one** reason — "off-limits".

**That is the bug this product exists to prevent.** A human approving a dangerous action while being shown only the first thing that made it dangerous is barely better off than not being asked at all.

**Rule:** tier and reasons are computed independently. The tier decides **whether** we interrupt; the reasons decide whether the human can **understand** what they are approving. An action carrying four hazards lists four reasons.

**Guard:** `tests/replit-acceptance.test.ts` asserts at least three reasons are present, and that the plain-language ones ("off-limits", "cannot be undone") actually appear.

---

## Standing constraints

- **Never restart the Gateway** — owner-only.
- Three hardening changes are written but inert until Jeff restarts: `fs.workspaceOnly=true`, qwen web deny, Telegram groups deny-by-default.
- **Keep the risk classifier in-house.** Do not delegate it to a fresh-context subagent — the threshold judgements are subtle and the rationale must persist.
