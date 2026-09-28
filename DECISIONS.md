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

## D17 — Single instance: a lock file, and refusal rather than a second agent
**2026-09-27**

**Decision:** a second launch **refuses and exits**. It does not start a second agent, and it does not try to steal focus from the first.

**Why this is a safety property and not tidiness.** Two agents sharing one state directory, one set of credentials and one sidecar port is precisely the contention this project exists to prevent. There is already a precedent for how that ends: two Ollama servers on one machine contending for the same GPU produced crashes and hangs. Being a consumer app makes it worse, not better — the user will double-click the icon twice and not think about it.

**Mechanism: an exclusively-opened lock file** (`share_mode(0)`, handle held for the process lifetime) at `<state dir>\shell.lock`. Rejected alternatives:
- A **named mutex** would also work, but the lock file sits next to the state it protects, which makes it inspectable when something goes wrong.
- **Validating a stored pid** is the classic mistake — pids are reused, so a stale file can point at an unrelated process and either block a legitimate launch or permit a duplicate.

The chosen mechanism needs no cleanup: **Windows releases the handle when the process dies, including when it dies badly.** There is no stale lock state to reason about.

**Verified at runtime, not just in a test:** the first instance ran with a heartbeating sidecar; the second printed `Capybaras is already running (lock held at …)` and exited. Exactly one shell, one sidecar, both cleared on exit.

**Known gap, stated plainly:** the plan's exit criterion said a second launch should *focus the first window*. It exits instead. Focusing requires an IPC channel to the running instance, which does not exist yet — the sidecar will own IPC later, and this can be revisited then. **Refusing to start a second agent is the safety-critical half and it works; focusing is a convenience and it is deferred.**

---

## D18 — One capybara, duplicated six times (Jeff's decision)
**2026-09-27**

**Decision: there is ONE character design.** It is duplicated six times, not drawn six times. The six names stay; the graphical personalisation does not extend to six different animals. Accents and labels do the differentiating.

**Proposed by Jeff, and it is the right call — for three reasons, only one of which is cost.**

**1. It is the honest picture.** The six agents are one piece of software with six jobs. Six visibly different animals would illustrate a difference between them that does not exist. Repetition of a single character says what is actually true.

**2. It is how mascot recognition works.** TunnelBear has one bear. Mailchimp has one Freddie. Duolingo has one owl. Recognition compounds through *seeing the same form again* — six designs would dilute the brand six ways rather than building it. `BRAND.md` previously claimed "we have a cast, because we have a cast"; that line is now corrected, because it argued for the expensive path on aesthetic grounds when the cheap path is also the more truthful one.

**3. It removes the exact blocker the mascot spike found.** Spike 003 established that generation cannot be shipped because the same prompt produced two different capybaras — continuity drift, with 24 mutually consistent poses required. **One character × four states is 4 poses.** The hard requirement drops by roughly 85%, from "a large illustration commission" to "a small, standard one."

**How the six stay distinguishable** — this is the part that must not be skipped, because the entire point of the herd is seeing *who is doing what*:

| Differentiator | Note |
|---|---|
| **Name label** | Always visible; the primary identifier |
| **Accent colour** | One per capybara. The six non-coral palette colours, one each — the scheme is exhausted exactly, with nothing spare to misuse. **Coral remains reserved for "needs you"** and is never a character accent. |
| **Fixed position** | A permanent slot in the row makes the layout spatially learnable |
| **State** | Dozing / listening / working / needs-you already separate them at a glance |

**The sign moves from a character's prop to a state's symbol.** It was Zeca's; it is now simply the visual definition of *"needs you"*, raised by whichever capybara needs you. Two wins: the most important moment in the interface is **always drawn identically** and therefore recognised instantly rather than re-read — and nothing has to be multiplied per character.

**Total art requirement: one character × four poses + one sign.**

**What this does not change:** the names, the jobs, the four states, the palette, and the rule that "needs you" is the loudest thing in the app. The herd still makes the architecture legible. It now does so through **count, accent and state** rather than through six distinct designs.

---

---

## D19 — Grant persistence: stored plainly, and it FAILS CLOSED
**2026-09-27**

Grants now survive a restart. `grants.json` lives beside the other state in `<state dir>` (D15), and the decisions below matter more than the storage mechanism.

**1. Persistence fails CLOSED, and this is the whole design.** A missing, unreadable, corrupt or unknown-format file yields an **empty** store — which means everything asks again. A persistence bug must never be able to *grant* authority. The failure direction is the point: losing grants costs a user a few extra clicks; gaining grants costs them the guarantee.

**2. Restoring a grant applies the SAME rules as recording one.** `GrantStore.restore()` is not a looser path. It re-checks tool, target, wildcard rejection, human provenance and expiry. A grants file is a file: it can be hand-edited, truncated, or written by an older version. **An entry that would not have been acceptable as a fresh human approval is not acceptable as a restored one** — otherwise the file becomes a way to obtain authority the approval path would have refused.

**3. Expired grants are refused, not resurrected.** Loading does not revive them.

**4. Writes are atomic.** A temporary file plus a rename, so a crash mid-write cannot leave a half-written grants file. On the same volume the replacement is atomic: the file is either wholly the old version or wholly the new one.

**5. The store stays pure.** `grants.ts` holds no filesystem code; `grant-file.ts` is the only module that touches disk, and the sidecar wires the two together. This kept all 42 existing policy tests untouched and made the persistence layer testable in isolation.

**6. Revocation is reachable from the interface, not only from the file.** `grants.revoke` was a protocol message with no caller until the shell grew `revoke_grant` and the interface grew a list with one-click *Forget* (2026-09-28). Deleting `grants.json` still works and is deliberately supported — but a permission whose only withdrawal route is editing a JSON file is not really in the user's hands. The proof is behavioural, not declarative: `tests/protocol.rs` revokes a grant and then makes the same action **ask again**, rather than trusting the reply that said it was removed.

**Verified end to end, not just in unit tests:** a Rust integration test runs the sidecar **twice against the same state directory**. Session one approves a file write and asks for it to be remembered; session two is a fresh process and proceeds **without asking**, citing the earlier approval. The same test then confirms a hard gate still asks — with a grants file sitting on disk.

**Still open from D15:** whether the state directory survives MSIX virtualisation. That question is unchanged, and this file now sits wherever the rest of the state does.
---

## D20 — Skills are declared, and a skill that forgets to declare itself will not build
**2026-09-27**

A skill suite is a **tool surface**, and the tool surface is the security boundary. Adding skills is therefore a safety-relevant act, not a feature toggle. `docs/skills.md` works this out in full; the decisions are these.

1. **One declarative manifest is the source of truth.** Today the tool knowledge is scattered across a `VERBS` table (headlines), a `TOOL_OWNER` table (the herd) and the classifier (tiers). Three lists that can disagree is three chances to be quietly wrong. They become one registry that classification reads.

2. **A missing field fails the build.** Not "assume safe". This project has already been bitten once by a lookup that returned nothing and was silently skipped — it produced a card with no styling that looked *almost* right. The same failure in a skill table would produce a tool that is almost safe. The failure mode is identical, so the mitigation is identical: refuse to build.

3. **The declared tier is a FLOOR, never a ceiling.** Escalation on reversibility, blast radius, taint and protected targets still applies. A manifest cannot make something quieter than the classifier would.

4. **Unknown tool still fails to `confirm`.** The permissive fallback is the one thing that must never exist, or a manifest that fails to load becomes an open door.

5. **We will NOT implement manipulation libraries in the safety core.** The gate classifies and interrupts; it does not parse PDFs. Every line of parsing code in the trusted path is attack surface inside the thing that is supposed to be trustworthy, and PDF parsers are a historically reliable source of memory-safety bugs.

6. **Some skills need post-conditions, not just tiers.** Redaction is the example: applying a black rectangle leaves the text intact underneath and extractable, and the document *looks* redacted. So a redaction skill must verify the content is gone by re-extraction and say so in the receipt. "Redacted 4 passages" is not enough; "removed the underlying text, verified" is.

7. **The combination matters more than any single skill.** Read untrusted content + hold file access + send externally is an exfiltration path; any two are survivable. `readsUntrusted` is declared per skill and must taint the process. **How far to take taint tracking across a session is NOT YET DECIDED** — flagged rather than guessed, because a weak mechanism in a load-bearing place is worse than an acknowledged gap.

**First suite: the PDF family** (`docs/skills.md` §3), classified read-silent, create-notify, mutate-confirm, and attach/decrypt at hard gate.
## D21 - The gate governs actions, not speech (recorded from Jeff's conversation, 2026-09-27)

**Where this came from.** Jeff talked with someone at a pharmacy about whether an AI
should explain how to make things like thermite and napalm, when the same material sits
in library books. His position: if it is on a shelf, restricting it is theatre. He asked
me to keep it in mind for this product.

**The architectural fact, which settles most of it.** Capybaras' gate is about
*actions*: it interrupts `db.delete`, `fs.delete`, `payment.send`. It has never claimed
to police what the agent will **explain**. So the honest scope statement is that this
product does not filter content, and adding a filter would not strengthen the promised
guarantee -- it would blur it. A local agent that can read your files has no business
being a censor.

**The line we do hold: explain versus build.** Describing chemistry is speech. Producing
a working procedure tailored to what the user has to hand, plus target selection or
evasion advice, is capability amplification -- and that is where a line is defensible.
That line is about *facilitation*, not about knowledge.

**The strongest argument against Jeff's position, recorded because it is not obvious:**
the information is not the variable, the *friction* is. A library requires knowing what
to look for, going there, and synthesising it. A model collapses all of that **and
iterates** -- "mine did not ignite, why?" That is a tutor rather than a book: a
difference in degree large enough to matter, not a difference in kind. It is worth
knowing before arguing that nothing has changed.

**Why this is written down rather than left implicit.** This is exactly where a product
drifts. The next person handed a hard question will reach for a blocklist, and without a
recorded reason they will be right to. The reasoning is the durable part, not the rule.

**Explicitly NOT decided here:** whether *some* friction is worth adding -- a stated
intent step, an age gate. That is a values call, it is Jeff's, and it is reversible. This
entry records the scope of the *current* design, not a verdict on anyone else's.

## D22 — The meter reports every call, and pushes rather than being polled
**2026-09-28**

`usage.rs` (M5) knew how to count, format and hold a session. Nothing read it: `lib.rs` declared the module and nothing else. The shell now holds the meter, exposes it to the interface, and emits `capybaras://usage`.

**1. Every finished call is reported, not only the calls that move a total.** The natural optimisation — diff the snapshot, emit on change — is wrong here, and `usage.rs` already says why: a request that fails after the provider has begun generating may return **no usage at all**. That call changes nothing, and *standing still is the evidence of the failure*. Suppressing "nothing changed" would suppress exactly the message a person needs to see. The price is one small JSON message per call, on a channel the app already owns, against a call that just crossed the network.

**2. The return value and the emitted snapshot are the same read.** `record_model_call` records, takes one snapshot, returns it and notifies with it. Two separate reads would let the caller and the interface disagree about the same call.

**3. The seam is one function, and it is named as the only one.** Anything that completes a model request reports through it. A path that forgot to would make the meter silently wrong rather than visibly broken — the failure mode `usage.rs` warns about in its own header.

**4. Push, not poll.** The interface reads `usage_status` once on load and is kept current by the event. The status panel keeps polling, because a sidecar can die without telling anyone; a spent figure cannot drift that way.

**What this does not do.** There are no model calls in the app yet, so nothing exercises the seam at runtime. It exists and is tested *ahead* of its first caller, deliberately, so that the first model call cannot be written without one.

---

## D23 — The card states each hazard exactly once
**2026-09-28**

Found by driving the acceptance scenario through the real interface: the Replit
card listed **five** reasons, and two of them restated hazards it had already
stated.

- `"this target is marked off-limits"` (the protection policy) and `"This target
  is marked off-limits."` (the classifier) both survived, because the gate merged
  the two layers with an exact-string `Set` and the pair differs by a capital
  letter and a full stop.
- `"This cannot be undone."` and `"This cannot be undone, and it affects many
  things."` both survived, because the classifier pushed the *combination* as a
  hazard of its own on top of the two it combined.

**Decision.** D10 is one reason **per hazard**, and a repeated line is not a
hazard — it is noise, and noise on the warning list is precisely what teaches
someone to stop reading it. So: the merge compares a **normalised** form (case,
whitespace, trailing punctuation) and a combination that only restates its parts
escalates the tier **without** adding a line. The acceptance card now states
exactly the three reasons `docs/acceptance-test.md` documents.

**Rejected:** dedupe on exact strings only (leaves the near-duplicates).
**Rejected:** drop the policy layer's reasons entirely (loses the *policy*
explanation when a freeze or a protected target is declared, which the classifier
cannot know about).

---

## D24 — A raised sign is lowered by a human's answer, and by nothing else
**2026-09-28**

The herd could contradict the card. With an approval pending, a **second action
owned by that same agent** lowered its own sign — `beginWork`'s guard only
protected *other* agents — and `release` conflated two different events: "work
finished" and "a human answered". The card stayed on screen while the herd showed
nobody needing anyone.

**Decision.** Two operations, not one. `release` means work finished and may only
return `working` to `listening`. `standDown` means a human answered and is the
only thing that clears `needs-you`. `beginWork` never overwrites a raised sign,
including for its own owner. `needs-you` is the loudest thing in the interface;
nothing may quietly switch it off.

**Consequence for the tests:** the old assertion that `release` lowers the sign
encoded the defect, so it was corrected to `standDown` rather than kept — with
the reason written beside it, so the change is visible rather than silent.

---

## D25 — The first model call lives in the shell, and the catalog is mirrored rather than shared
**2026-09-28**

There are now model calls in the app. This records where they sit, and pays for the
one real cost of putting them there.

**Decision: the SHELL (Rust) makes the provider calls.** Not the sidecar, not the
webview.

The key is authority, and M5-onboarding.md §3 settles where it may go: the shell
holds it, and the token is never to reach the protocol stream — a token on the wire
is a token the sidecar can then keep. The sidecar is the component that runs
alongside untrusted content; the webview is the component an XSS reaches (T3). So
the call is made by the smallest trusted surface available, using the same OS-TLS
`reqwest` client the token exchange already uses.

**The reporting seam is `chat::perform`, and it is structural.** It makes the
request, then reports through one closure — on success and on failure alike — with
`record_model_call` as the destination. D22 requires a failed call to be reported,
because a call that consumed nothing is itself the signal; making the report part of
the only path through the function is how that stops being something a later caller
can forget. A test drives the whole composition (`report_of` → `record_and_notify`)
against a stub, so "the meter moves even when the call failed" is proven with no
network and no money.

**The catalog is parsed twice, deliberately, and that is a real cost.**

- `src/onboarding/models.ts` is the catalog's display contract and it is pure: it
takes a `fetch`, so it can be tested with no network. It has no notion of a key.
- The fetch needs the key, and the key cannot leave the shell, so the parsing that
  consumes the response has to exist on the Rust side: `src/catalog.rs`.
- The mirror is faithful on purpose — the same bounds (80 / 240), the same
  control-character flattening, the same display-fields-only guarantee, the same
  "an empty catalog is an error, not an empty menu" rule. The structural half is
  asserted in **both** languages (`the_catalog_exposes_display_fields_only` in Rust,
  "exposes display fields only" in TypeScript), so the property that matters cannot
  drift silently.

**Rejected: the sidecar making the call** — it would need the key, which is the
whole threat T12 names. **Rejected: the frontend making the call** — it would need
the key, from the origin T3 calls the likeliest foothold. **Rejected: shell fetches,
sidecar parses** — that puts the key or the raw provider body on the protocol
stream, which §3 forbids. **Rejected: a curated shortlist that lives only in
TypeScript** — the shell must choose a default from the live list, so the four
preferred ids travel with it, in the same order.

**Honest limit.** Two implementations of one convention can drift. The constants
and the structural test are duplicated so drift fails loudly rather than silently,
but a single source would be better and is not available while the key lives on the
Rust side. If the call ever moves, `catalog.rs` should be deleted, not kept.

---

## D26 — The spend cap: an open question, and what a decision has to cover
**2026-09-28**

M5's plan lists a "spend cap" (`docs/plans/next-steps.md`), and D7 says "a low
default monthly spend cap is on from day one" (2026-09-27). **Neither is true of
the code.** M5 was asked to settle the question, so this records what was found
rather than leaving a promise and an implementation quietly disagreeing.

### The finding, with the places to look

**There is no enforced cap. There is not even a wired display.**

| What | Where | What it actually does |
|---|---|---|
| The session counters | `apps/desktop/src-tauri/src/usage.rs:53` (`Totals::record`) | adds calls, tokens and cost. No comparison, no limit, no refusal |
| The "remaining" figure | `usage.rs:94` (`Credit::remaining`) | subtracts usage from credits, for display |
| The credit reading | `usage.rs:100` (`CreditSnapshot`) | holds a fetched balance, with a scope (account vs key) |
| The reader | `usage.rs:185` / `meter.rs:54` (`set_credit`) | stores a snapshot — and **has no caller anywhere** |
| The meter commands | `lib.rs:181` (`usage_status`), `lib.rs:195` (`record_model_call`) | read and record; neither consults a cap |
| The model call | `chat.rs:84` (`chat::call`) | builds the request and sends it. **No budget check on the path** |
| The only spending refusal | `chat.rs:139` | the **provider's** 402, turned into a plain sentence |
| The interface | `apps/desktop/web/app.js:358-364` | prints `spent` and, if a credit reading exists, a balance coloured by whether it is negative |

So the references a scan finds in `usage.rs` and `app.js` are **credit reading**,
exactly as the scan suspected: they present a number, they do not stop a call.

The second half of the finding is that even the *display* is unreachable:
`set_credit` has no caller, so `Snapshot.credit` is always `None` and the "left"
row never renders. The one place a user could have seen a limit is dead code.

### Why this is not an oversight in the code — and is still a gap in the product

`docs/plans/M5-onboarding.md` §4 decides the shape deliberately:

> a local cap is advice; a provider-side cap is a limit ... the default should be a
> provider-side limit, set during onboarding, with the app's own counter as a
> *warning* rather than a control.

That reasoning holds: T12 already says the agent can reach the key, and anything
that can reach the provider directly can spend past a counter the app keeps. So a
cap the app enforces by counting is **theatre against the threat the app exists
for**, and building one would make the product *claim* a limit it cannot honour.

But the plan's other half is unbuilt too: §2 step 5 says the window shows
"Connected" and **the monthly cap**, and §4 says a new user should *see the figure
before they can exceed it*. Today the window shows neither. A person can connect
and spend with no cap in force, no cap set, and no cap visible.

### Why nothing was implemented here

- **The repository specifies the opposite of a local cap.** Implementing one would
  contradict a recorded decision, not fulfil it.
- **There is no spec for the rest of the behaviour**: when the provider-side limit
  is read, what happens when it is reached, and what the app says. Inventing a
  policy on a screen whose whole job is honesty about cost is the wrong place to
  guess.
- It is not small: reading the key's limit means a new network call (`GET /key`),
  a new field on the meter, a caller for `set_credit`, and a way to test it
  offline. That is a milestone item, not a fix.

### What a decision must cover

1. **Which limit is the control**: the key's own limit (`GET /key` → `limit`,
   `limit_remaining`) or the account balance (`GET /credits`). Only a limit set on
   the key answers "what may Capybaras spend"; the account figure is shared with
   everything else on it, and `CreditScope` already carries that distinction.
2. **When it is read**, and how the figure stays honest: at connect, on each call,
   or on a schedule — and what is shown when the read fails.
3. **What the app does when the limit is reached**: refuse locally before sending
   (and say so plainly), or send and relay the provider's own 402 (which is what it
   does today). Refusing locally is a *courtesy*; the provider's refusal is the
   *limit*. The product must not confuse them.
4. **Whether onboarding sets the limit** or only explains where to set it — the
   OAuth request carries no limit parameter (`oauth.rs`), so any "set during
   onboarding" step is guidance, not configuration.
5. **What D7's sentence should say** once the answer exists, because as written it
   claims something untrue today.

**Until then, the honest statement is the one M5 already implies:** Capybaras shows
what it has spent; the only thing that limits spending is the limit on the key in
the user's own OpenRouter account, and the app does not yet show it.

---

## Standing constraints

- **Never restart the Gateway** — owner-only.
- Three hardening changes are written but inert until Jeff restarts: `fs.workspaceOnly=true`, qwen web deny, Telegram groups deny-by-default.
- **Keep the risk classifier in-house.** Do not delegate it to a fresh-context subagent — the threshold judgements are subtle and the rationale must persist.
