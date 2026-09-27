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

## D11 — Distribution corrected: the Store is free, and MSIX changes the service model
**2026-09-27**

Research is in `docs/research/packaging-and-signing.md`. It corrected two things already written into `BUILD-PLAN.md`, and surfaced one architecture change.

**1. The Microsoft Store developer account is now free.** Microsoft removed the fee for individual developers in September 2025, and for companies on 7 May 2026. Microsoft Learn now states: *"there are no registration fees for either account type."* **I had budgeted $19 and told Jeff so — the correct figure is $0.** Correction recorded because it was stated as fact.

**2. MSIX Store packages are re-signed by Microsoft**, which yields instant SmartScreen trust with no separate certificate. This makes the Store the **only guaranteed no-warning path**.

Correspondingly: **Azure Artifact Signing (~$10/mo) does NOT grant instant SmartScreen reputation.** Microsoft's own documentation contradicts the third-party marketing that claims otherwise. Third-party claim, debunked.

**3. Architecture change — MSIX has no Scheduled Tasks.** The Gateway's Windows auto-start is currently a Scheduled Task. Under MSIX that mechanism does not exist and must become an **MSIX Start-up Task**. This is a real change to how the Gateway is supervised at startup, not a packaging detail.

**Carrying real, unverified risk:** whether a bundled **Node sidecar** behaves correctly inside an MSIX *full-trust* package. The current evidence is a composite of separately-documented parts; no single source documents MSIX + Tauri sidecar + Store submission together. **The Phase 2 spike must test this case specifically**, before any packaging work begins.

---

## D12 — The Phase 2 spike: PARTIAL, and the honest reason why
**2026-09-27**

Spike code and findings: `spikes/001-sidecar-supervision/`. Question: when a supervisor spawns a Node sidecar and the supervisor dies, does the sidecar orphan?

**Verdict: PARTIAL.** Clean shutdown works. The orphaning question could **not** be answered from inside the agent's own process tree — this host anchors descendants with a Windows Job Object (the runtime runs `service-child-windows-job-anchor.js`), so there is no way to distinguish *"the product would orphan"* from *"the harness killed it."* **The instrument contaminates the measurement.** Recording that plainly rather than reporting a confident result the evidence does not support.

**1. The mechanism that would have answered the spike is the mechanism to build with.** A Windows Job Object with `KILL_ON_JOB_CLOSE`: when the supervisor dies, every process in the job dies with it. This is already a proven pattern in this exact stack — OpenClaw's own runtime supervises its children this way. Not an invention; a known-good approach applied deliberately.

**2. `child.kill()` is not a graceful shutdown on Windows.** It calls `TerminateProcess` — no `SIGTERM` handler runs. Verified in the spike: the sidecar's signal handler never logged. A Gateway that owns sessions, sockets and credentials must be asked to stop over IPC and given a bounded grace period **before** the force. A hard kill on every quit means risking half-written state.

**3. The first harness was confounded, and its output was contradictory.** The sidecar's stdout was piped to the supervisor, so the pipe broke when the supervisor died — a second death mechanism mixed into the test. The results table said "cleaned up" while a process sweep appeared to show survivors. Both were noise. Rebuilt with `stdio: 'ignore'` and file-based logging. **Bad instrumentation is not a finding**, and a contradictory result is a signal to check the instrument before believing the number.

**4. The MSIX full-trust question remains untested — and this is the important one.** The unverified risk inherited from D11 was whether a bundled Node sidecar behaves correctly inside an MSIX full-trust package. It cannot be tested on this machine right now:
- **Developer Mode is off** (`AllowDevelopmentWithoutDevLicense` unset), so a loose unsigned MSIX cannot be installed.
- Trusting a test certificate requires **elevation**, which this channel does not have.

The highest-risk unknown of the whole packaging plan is therefore still unknown — for an environmental reason, not a technical one. **This is a user action, not an engineering one:** either enable Developer Mode, or run one elevated shell when the packaging phase begins.

**Also noted for the shell work:** the Tauri CLI is not installed here (`no such command: tauri`), and the Windows SDK's `makeappx`/`signtool` are present only under the **arm64** kit path. Both are Phase 3 setup items, not blockers now.

---

## D13 — MSIX full-trust + bundled Node sidecar: VALIDATED, and two rules it imposes
**2026-09-27**

Spike: `spikes/002-msix-sidecar/`. This closes the highest-risk unknown inherited from D11 — whether a bundled Node sidecar works inside an MSIX *full-trust* package. No single source documented MSIX + a desktop shell + a Node sidecar together, so it had to be tested rather than cited.

**Verdict: VALIDATED.** Built a real package (stdlib-only Rust launcher → bundled `node.exe` → bundled `sidecar.mjs`, 34 MB MSIX). A full-trust MSIX app with package identity launched, resolved its own app-local `node.exe`, spawned it, and Node ran — `node spawn: OK (exit 0)`, `sidecar ran under full-trust MSIX: v24.16.0`. Evidence is the app's own output, not inference.

**Rule 1 — never resolve paths from `cwd`.** The packaged app launched with `cwd = C:\Windows\system32`. Any code that builds paths relative to the working directory breaks under MSIX. Paths must come from `process.execPath`, the module URL, or an explicit configured root.

**Rule 2 — decide the state directory deliberately.** MSIX **silently virtualizes AppData writes.** The launcher wrote to `%LOCALAPPDATA%\CapybarasSpike\`; the bytes landed in `%LOCALAPPDATA%\Packages\<PFN>\LocalCache\Local\CapybarasSpike\`. The process believes it is writing to normal AppData — `process.env.LOCALAPPDATA` reports the plain path, and `USERPROFILE` too. Good for isolation; bad for anything the user or a support process must find. **Decide whether state lives in the virtualized store (and expose an "open data folder" action) or is redirected explicitly.**

**One measurement could NOT be taken honestly, and is recorded as such:** `app dir writable` read `yes`, but the test ran from a **loose layout registered out of the workspace folder**, which is writable by definition. A real install lives in the read-only `WindowsApps`. **Re-take that reading against a genuine install when packaging begins.** Not dropped, not overstated.

**Install mechanics, and where elevation is actually required:**
- Unsigned MSIX is rejected (`0x800B0100`) — **Developer Mode permits sideloading, not unsigned packages.** Corrected a wrong assumption.
- Self-signed works for signing, but deployment requires **machine**-scope trust; `LocalMachine\TrustedPeople` returns `E_ACCESSDENIED` (`0x80070005`) without elevation. User-scope trust is insufficient (`0x800B0109`).
- **Loose-layout registration (`Add-AppxPackage -Register AppxManifest.xml`) works unsigned and unelevated** — this is the dev loop, and it produced the evidence.
- **Distribution is unaffected:** Store packages are re-signed by Microsoft, so no end user touches a certificate. The elevation cost is a **developer-machine** cost only.

**Recommendation:** ship MSIX, and encode Rules 1 and 2 in the Gateway before the shell is built.

---

## D14 — Bayesian mathematics in the app: no in the safety path, yes in exactly one place, not yet
**2026-09-27** (asked by Jeff: *"utilize Bayesian mathematics in the app if possible but only if it makes sense — if it doesn't then don't do it"*)

**Verdict: the idea is not silly, but it would be actively harmful in the core, and the one place it genuinely fits is not worth building yet.** Recording it as considered-and-deferred with the constraints it would have to satisfy, rather than silently dropping it.

### Why it must NOT go in the core

The classifier's guarantees are **structural properties of a rule system**, provable by inspection:

- escalation-only — a classification can never become *less* cautious
- unknown tools **fail safe** to `confirm`
- **a hard gate is never satisfied by memory**, however many times it was approved
- the model has **no code path** that creates a grant

Replace the tier decision with a posterior probability and **none of those can be proved any more.** They become statements of the form "probably true" — which is a downgrade in a safety product, not an upgrade. A user cannot audit "the posterior exceeded 0.7." **The product's entire value is that a human can read the rule and know what happens. Probability is the thing we are protecting people from, not the tool we protect them with.**

### Where it legitimately fits

**1. Escalation-only soft-signal scoring.** When no hard rule fires, combine weak signals (unusual hour, unseen target, unfamiliar action shape, atypical scope) into a posterior that can **only raise** a tier. This is safe in one direction by construction: a false positive costs one click, and a false negative is impossible because the scorer cannot de-escalate anything. This is the only shape in which probability belongs in the decision path.

**2. Threshold justification.** The defaults ("mass action" at some row count, scope limits) are currently **chosen, not derived.** Bayesian reasoning is genuinely the right tool to *state the priors and the asymmetric loss function* behind each one: missing a destructive action is catastrophic, an unnecessary prompt costs one click. That asymmetry is why the defaults are conservative, and writing it down makes them defensible and tunable. **This is a document, not runtime code.**

### Constraints if (1) is ever built

- The scorer **cannot** lower a tier, satisfy a grant, or affect a hard gate. Escalation-only, enforced structurally.
- Its output is an **input to a deterministic rule**, never a replacement for one.
- It must be **off by default**, behind a flag, with the deterministic behaviour as the default path.
- Its reasons must render in plain language like every other reason. "This is unlike anything you have approved before" — never a number the user must interpret.
- The acceptance suite must pass with it both on and off.

### Timing

**Not now.** The deterministic core is not yet proven end-to-end — M4 (the Replit gate in a real UI) is still ahead. Adding probabilistic machinery before the guarantee is demonstrated would dilute the guarantee *and* add false confidence. Sequence it **after** the acceptance test is green, if it still looks worth it then.

### One distinction worth keeping

Jeff's standing directive is that **I reason and communicate with calibrated uncertainty**. That is a good idea and stays. This decision is about something different: whether the *product's safety engine* should be probabilistic. Same word, different question. **Calibrated confidence in how I talk to Jeff: yes. Probabilistic permissions in the app: no.**

---

## D15 — State directory: explicit, overridable, and surfaced to the user
**2026-09-27**

Spike 002 finding: MSIX silently virtualises writes to `%LOCALAPPDATA%`. A packaged app that writes to `%LOCALAPPDATA%\Capybaras\` will find its bytes in `Packages\<PFN>\LocalCache\Local\Capybaras\`, and **the process cannot detect the difference** — `process.env.LOCALAPPDATA` still reports the plain path.

**Decision.** State resolves through one function, in this order:

1. `CAPYBARAS_STATE_DIR` — explicit override, used by tests and development.
2. `%LOCALAPPDATA%\Capybaras` — the default.

And the resolved path is **shown in the shell UI**, because a state directory the user cannot find is a state directory they cannot back up, inspect, or delete.

**Open question, to be measured not asserted:** whether a chosen *user-visible* path survives MSIX virtualisation, or whether only the package-private store is dependable. Known-folder redirection is not documented clearly enough to settle by reading. This is a packaging-phase measurement, and it is recorded here so it is not quietly forgotten.

**What this does not yet solve.** The override is an escape hatch, not an answer. If MSIX virtualisation turns out to swallow the default path, the fix is to move to a location that is genuinely visible, and to say so in the UI — not to hope the user finds `LocalCache`.

---

## D16 — Supervision verified: no orphans, and graceful stop reaches the sidecar
**2026-09-27**

Spike 001 left one thing unmeasured: non-orphaning, because the agent's own runtime anchors descendants with a Job Object, making harness death and product death indistinguishable. **M3 Step 6 re-took that measurement from outside the process tree.**

Launch was a **Scheduled Task** — outside the agent's job — with the working directory forced to `C:\Windows\System32`, which tests the spike-002 path rule at the same time.

**Result 1 — path resolution survives `cwd = System32`.** The sidecar started and reported `cwd=C:\Windows\System32`. Had any path been resolved from the working directory, it would not have started at all.

**Result 2 — no orphans.** The shell was hard-killed (`taskkill /F`, so no handlers, no cleanup, no chance to be polite). Five seconds later: **zero surviving sidecar processes.** The `KILL_ON_JOB_CLOSE` job object did exactly what it was chosen for.

**Result 3 — a graceful stop runs the child's own handler.** An automated test asserts that `stop()` reaches the sidecar over stdin, that the sidecar exits 0 on its own rather than being terminated, and that its shutdown handler logged. On Windows `Child::kill()` is `TerminateProcess` and runs no handler, so this is a real assertion rather than a formality.

Verification is scripted and repeatable: `apps/desktop/scripts/verify-supervision.ps1`, plus `cargo test` in `apps/desktop/src-tauri`.

**Honest caveat:** in this development run the sidecar resolved to the Node on `PATH` (`C:\nvm4w\nodejs\node.exe`), because a debug build has no bundled `node.exe` beside the executable. The release bundle will place it there. The resolution *order* — executable directory first, `PATH` only as a development fallback — is what the shipped build relies on, and it is unchanged.

---

## Standing constraints

- **Never restart the Gateway** — owner-only.
- Three hardening changes are written but inert until Jeff restarts: `fs.workspaceOnly=true`, qwen web deny, Telegram groups deny-by-default.
- **Keep the risk classifier in-house.** Do not delegate it to a fresh-context subagent — the threshold judgements are subtle and the rationale must persist.
