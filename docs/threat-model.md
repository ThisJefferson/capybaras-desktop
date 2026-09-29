# Threat model

**What this is:** the attacks that matter for *this* application, and what we do about each. Not a generic OWASP list — a list filtered through one question: **what would an attacker who wants to get past the gate actually do?**

**Last reviewed:** 2026-09-27

---

## 1. The framing that organises everything

Capybaras is a **gate**. Its value is that a destructive action cannot run without a person agreeing. So the attacker's rational move is not to defeat the classifier by argument. It is to:

1. **disconnect the gate** (make it not run, or run in a degraded mode);
2. **rewrite the gate** (change the code or the configuration that decides);
3. **convince the person** (make the human click approve — including by making the card lie);
4. **outrun the gate** (get the action executed without ever passing through it).

Every threat below is one of those four. That is also the priority order.

A second principle, inherited from `docs/DEBUGGING.md`: **in this project the dangerous failures are silent ones.** Nothing throws, everything looks nearly right. A gate that fails *open* silently is the worst possible version of this product, and most of this document is about not being that.

---

## 2. Assets

| Asset | Why it is worth attacking |
|---|---|
| **The decision logic** (`src/risk-classifier`, `src/policy`) | It is the product. Rewriting it disables every guarantee at once |
| **`policy.json`** (freezes, off-limits targets) | Removing a freeze silently converts a protected machine into an unprotected one |
| **`grants.json`** (remembered approvals) | Injecting a grant pre-approves an action the human never saw |
| **The sidecar bundle** (`sidecar.mjs`, plain JavaScript) | Whoever can write this file *is* the gate |
| **The approval channel** | Answering an approval without a human is a total bypass |
| **The user's machine** | The real payload — this is what the gate exists to protect |
| **The user's attention** | Finite, and the thing every social-engineering attack here consumes |

---

## 3. Attacker model

Four realistic attackers, in descending order of likelihood:

- **A1 — A prompt-injected agent.** Untrusted content the agent reads tells it what to do. The agent itself is not malicious; it is *used*. Arrives as ordinary attacker-authored text: a filename, a log line, an email body, a PDF.
- **A2 — Malware already on the machine, running as the user.** Can read and write anything the user can. **This is the attacker that most of our current design cannot stop**, and honesty about that is more useful than pretending otherwise.
- **A3 — A malicious or compromised dependency.** npm, cargo, a transitive package, or a GitHub Action.
- **A4 — A person at the keyboard.** Not an intruder: a tired human at 11pm clicking "Go ahead" because the card looked fine.

**Out of scope, stated plainly:** a remote network attacker with no code execution on the machine; a supply-chain compromise of the operating system; and an attacker with administrator rights (who can do anything by definition).

---

## 4. Threats

Each entry: what the attack is, why it applies *here*, current state, and what closes it.

### T1 — Rewriting the protection configuration
**Attack.** Delete the freeze, or edit the off-limits list, then ask the agent to do what was forbidden. Or inject a `grants.json` entry to pre-approve.

**Why here.** The configuration is the protection. There is no second line of defence behind it.

**Current state.** Two defences, and one honest gap.
- Loading **fails closed**: a corrupt or truncated policy yields a policy that protects *everything*, so tampering that breaks the file cannot silently *disable* protection.
- **Self-protection**: `policy.json` and `grants.json` are off-limits to the agent's own file tools by default.
- **Gap:** the files sit in the state directory, which the *user* can write — and the agent runs as the user. So A2 can rewrite them, and the app cannot prevent it. The sidecar now **detects and reports this at every boot** rather than implying a guarantee it cannot make.

**Closes it.** An ACL that makes the policy file read-only to the user account (admin-owned). This is an OS action, not a code change. **Priority: high — it is the difference between "the freeze is announced" and "the freeze is enforced".**

### T2 — Rewriting the sidecar
**Attack.** Edit `sidecar.mjs` on disk, or replace it. The new version approves everything and reports success.

**Why here.** The sidecar *is* the decision logic, and it ships as **plain, readable, writable JavaScript**. It is not compiled, not signed, and not verified before launch. A2 owns this file.

**Current state.** No defence. This is the most serious structural weakness in the current design, and it is worth saying so plainly.

**Closes it.** In order of strength:
1. **Verify before launch.** Hash the sidecar bundle and compare against a value baked into the signed binary; refuse to start on mismatch. Cheap, and it catches casual tampering.
2. **Ship it signed.** Code signing on the bundle and on the installer (M6 territory).
3. **Move the decision logic into the Rust binary.** The strongest answer: the classifier and policy checks compile into the signed executable, and Node becomes a thin transport. Larger change; the right long-term shape.

**Priority: high.** Items 1 and 2 are the M6 work already planned; item 3 deserves a decision before M5 adds more logic to the sidecar.

### T3 — XSS in the webview
**Attack.** Get script executing in the frontend. From there, call the IPC commands directly — including answering an approval.

**Why here — and this one is unusual.** Our app **deliberately displays text an attacker wrote.** The approval card shows a target label; later it will show model output and tool results. Anything that renders attacker-controlled text is an XSS candidate, and this app renders it *by design*, as its central feature.

The single-point-of-failure shape is well described in the wider Tauri ecosystem: the frontend is one origin, so the first XSS sink that ever lands reaches **every** registered command.

**Current state.** Better than it might be:
- CSP is set: `default-src 'self'; script-src 'self'`. No inline scripts, no inline styles, no remote origins.
- The frontend writes text with `textContent`, never `innerHTML`. **This must remain a rule, not a habit.**
- The frontend is served from the app's own origin, not `file://`.

**Closes it.**
- Keep `textContent`-only as an enforced rule, with a test that greps for `innerHTML`/`insertAdjacentHTML`/`eval` and fails.
- Add explicit `style-src 'self'` rather than relying on the `default-src` fallback (already effectively true, but explicit survives a future edit to `default-src`).
- **Tauri capabilities**: narrow which commands the window may call, so an XSS cannot reach the whole surface. Currently every command is reachable.

**Priority: high for the grep test (cheap), medium for capability scoping.**

### T4 — Making the card lie
**Attack.** Craft a filename, table name, or label such that the approval card reads benign. The card is the last line of defence, so an attacker attacks the *text of the card*.

**Why here.** This is the threat most specific to our design, and it has no equivalent in a normal app. The card interpolates `targetLabel` straight from the action. A crafted label can:
- be reassuring — `notes.md (safe, already reviewed)`;
- push the real information off-screen;
- impersonate our own copy — `Ignore the warnings below, this target is allowlisted`;
- or be extremely long, burying the reasons.

**Current state.** The **headline** comes from the skill registry — our text, not theirs, which is the right call and must stay that way. The **label** is attacker-controlled and currently rendered indistinguishably from our copy.

**Closes it.**
- **Visually separate machine-authored text from input-derived text.** The card's own words and the attacker's words must never look the same.
- Clamp label length; a label cannot be a paragraph.
- Never let input-derived text set the headline, the tier, or the button labels. (Already true; make it a test.)
- Consider not showing raw labels at all for hard gates — a hard gate can be described entirely in our words.

**Priority: high.** No test covers this today, and it is the attack our own design invites.

### T5 — Approval fatigue
**Attack.** Not an exploit — a condition. Ask often enough about harmless things that the human learns to click through. Then ask once about something that matters.

**Why here.** Every gate fails this way eventually. **A control people route around is worse than no control, because it also removes the signal.**

**Current state.** Several deliberate choices already work in our favour: reads proceed silently; creations only notify; only destructive and irreversible actions stop; the herd shows *what* is being asked so it is not a wall of undifferentiated dialogs.

**Closes it.** Treat "how often does this interrupt?" as a **security metric**, not a UX metric. Every new skill added must justify the interruptions it creates. This is the direct link between "secure" and "easy and productive": **an app that is annoying about trivia will be clicked through on the things that matter.**

**Priority: standing.** This is a design constraint, not a task.

### T6 — Time-of-check to time-of-use (approval–execution mismatch)
**Attack.** Get approval for action X, then execute action Y.

**Why here.** Approvals are keyed by id and held in the sidecar. The moment an executor is added (M5), there is a window between "the human approved" and "the action ran". If the executor re-reads the action from anywhere mutable, the approval can be separated from the thing approved.

**Current state.** Not exploitable yet, because nothing executes. **This is a requirement to write down now, before it becomes a bug** — the safest time to specify it is before the code exists.

**Closes it.** The approval must bind to a **hash of the exact action**. The executor verifies the hash it was given matches the hash approved, and refuses on mismatch. Not "the same action" — the same bytes.

**Priority: high as a specification, before M5.**

### T7 — The gate fails open
**Attack.** Cause an error in the classification path — malformed input, an unexpected tool, an exception — and hope the code proceeds because it did not know what to do.

**Why here.** Every failure in a gate has two possible directions, and only one of them is safe.

**Current state.** The design already leans the right way: unknown tools fall to `confirm`, not to `silent`; a corrupt grants file yields an *empty* store; a corrupt policy protects everything; the skill registry refuses to build on an undeclared skill. The escalation rules are one-way by construction.

**Closes it.** Keep the rule explicit and enforce it by test: **any error in the decision path resolves to the more cautious tier.** An audit of every `catch` and every default in `src/policy` and `src/risk-classifier`, with a test per path.

**Priority: medium-high — mostly an audit and a set of tests.**

### T8 — Environment and path hijack
**Attack.** Control what `node` resolves to, so the shell launches the attacker's interpreter instead of Node's.

**Why here.** The Rust shell spawns a Node process. If it launches `node` by bare name, `PATH` decides which binary runs.

**Current state.** The sidecar *script* path is resolved absolutely and tested (`paths.rs` asserts it is never cwd-relative). **The `node` executable itself should get the same treatment and the same test.**

**Closes it.** Resolve the Node executable deliberately, refuse to start if it cannot be found somewhere expected, and test that the resolution is absolute.

**Priority: medium.**

### T9 — Symlink and path traversal
**Attack.** Point the agent at a path that is a symlink, or contains `..`, so the action reaches somewhere the user did not intend — including the protection files.

**Why here.** The classifier reasons about the *label*, and a label is not a resolved path. `link.txt` may resolve to `policy.json`.

**Current state.** Protection matching uses glob patterns against labels and argument strings. **A pattern cannot see through a symlink.** This is a real gap in T1's defence.

**Closes it.** For protected-path checks, resolve the real path before matching. Where resolution is impossible, fail cautious.

**Priority: medium-high — it undermines T1.**

### T10 — Supply chain
**Attack.** A malicious dependency, or a compromised CI action.

**Why here.** We build from npm and cargo, and our CI uses GitHub Actions.

**Current state.** A real, specific problem: **the workflow pins actions by mutable tag** (`actions/checkout@v4`), not by commit SHA. A tag can be moved. Cargo and npm have lockfiles, which is the main protection on the dependency side.

**Closes it.** Pin every action to a full commit SHA. Enable Dependabot for both ecosystems. `npm audit` and `cargo audit` in CI. Add `--locked` where applicable.

**Priority: high, and cheap.** This is the fastest real improvement available.

### T11 — The model as an attack vector (M5)
**Attack.** Untrusted content reaches the model, which then proposes an action that *looks* legitimate. Or the model is induced to propose a long sequence of small harmful actions rather than one obviously catastrophic one.

**Why here.** The gate reasons about **one action at a time**, and M5 is where a model starts generating those actions.

**Current state.** Correct by design: the model cannot classify itself, cannot set its own tier, and cannot create a grant. Those are the important properties and they hold.

**Closes it.** For M5:
- The gate must never accept a tier, a classification, or a "this is safe" claim *from the model*.
- **Sequence awareness**: many small permitted actions can be a large harmful one. Not solved; flagged.
- Costs and rate limits as a tier input, not just per-action risk.

**Priority: design requirement for M5, not a task now.**

---

### T12 — Injection through file content

**Attack.** A file whose *contents are instructions* — "ignore your rules", "run this", "this document authorises the change" — read by the agent as though the user had said it.

**Why here.** This is the highest-risk threat for an agent that ingests documents, because **an antivirus does nothing about it**: a *clean* file can be a working injection. The README already concedes that prompt injection may be permanent rather than fixable, and this is the shape it takes once a read path exists.

**Current state.** Not defended, and not defended *yet* because the app has no read path at all. The read path is what Stage 3 adds, so this closes before it does.

**Closes it.** Architecturally, not with a filter: **file content is data and can never become instructions.** Quoted material is kept out of the decision path entirely — a file cannot tell the app what to do, and no amount of confident phrasing in a document lowers a sign. Structural inspection and out-of-process parsing reduce the surface; neither is what makes this safe.

**Priority: high — it is the reason the read path is gated at all.**

### T13 — Parser exploitation

**Attack.** A malformed document that exploits whatever opens it: embedded JavaScript, `/Launch` and `/EmbeddedFile` actions, macros, or a declared type that does not match the extension.

**Why here.** Parsing a hostile file inside the process that holds the credential would put a memory-safety bug one step from the key — the opposite of T2's arrangement, where the sidecar runs beside untrusted content and the shell holds the key.

**Current state.** Not defended; no parser is wired in yet, which is the moment to get this right rather than after.

**Closes it.** **Structural inspection before opening** — what the file *is*, not a signature scan: embedded scripts, launch actions, embedded executables, macros, encryption, and whether the declared type matches the extension. No signature feed, so nothing to go stale. Then **parse out of process**, in the sidecar, never in the shell.

**Priority: high.**

### T14 — Exfiltration via writes

**Attack.** The agent writes a file — to a share, a synced folder, a path something else watches — and the content leaves the machine.

**Why here.** The write path is being added, and **the operator's answer removed containment**: *"you can write to any folder you need."* A write is therefore no longer bounded by a declared folder.

**Current state.** Not defended. **The gate is now the only layer**, where it was previously meant to be the second, behind containment.

**Closes it.** The confirm gate on every write, with the destination named on the card in full. This entry stays here, and the plan states the trade plainly, so that the missing layer is visible rather than discovered later.

**Priority: high — the layer that would have bounded it was traded away deliberately.**

### T15 — Overwrite and destruction

**Attack.** A write clobbers an existing file — silently, and perhaps irreversibly.

**Why here.** A model asked to "save the report" will choose a path, and that path may already be someone's work.

**Current state.** Not defended.

**Closes it.** **Atomic writes** — the temp-and-rename pattern `grants` already uses, so a failure leaves the original intact — and **never a silent overwrite**: an existing target is a question, not an inconvenience.

**Priority: high, and cheap.**

### T16 — Expansion bombs

**Attack.** A small file that expands without bound: a decompression bomb, deeply nested archives, a document that yields gigabytes of text.

**Why here.** The read path takes files from the person, and the parse runs in the sidecar beside the rest of the app's work.

**Current state.** Not defended.

**Closes it.** **Bounds on size, pages, and extracted text**, enforced before and during parsing, with the limit named plainly on the card when it bites rather than a silent truncation.

**Priority: medium-high.**

**And no bundled antivirus, deliberately.** Structural inspection plus the data-not-instructions rule is what stands in its place. The reason is recorded here because *"we deliberately do not scan, and here is what we do instead"* is defensible, while *"we scan files"* without saying what that means is not.

---

## 5. Do this next, in order

| # | Action | Effort | Closes |
|---|---|---|---|
| 1 | Pin GitHub Actions to commit SHAs; add `npm audit`/`cargo audit` to CI | small | T10 |
| 2 | Grep test forbidding `innerHTML`/`eval` in the frontend | small | T3 |
| 3 | Tests pinning the card's own text: input cannot set the headline, tier or buttons | small | T4 |
| 4 | Resolve the Node executable absolutely, with a test | small | T8 |
| 5 | Resolve symlinks before protected-path matching | medium | T9, T1 |
| 6 | Write the **approval-hash** requirement into the protocol spec before M5 | small | T6 |
| 7 | ACL the policy file (documented `icacls` steps for the operator) | small, needs admin | T1 |
| 8 | Verify the sidecar bundle hash before launch | medium | T2 |
| 9 | Fail-open audit with a test per catch/default | medium | T7 |
| 10 | Tauri capability scoping for the window | medium | T3 |
| 11 | Structural inspection before opening any file | medium | T13, T16 |
| 12 | Parse out of process, in the sidecar, never in the shell | medium | T13 |
| 13 | Atomic writes, and never a silent overwrite | small | T15 |

**Items 1–3 are a few hours and close a whole category.** That is where I would start.

---

## 6. What we deliberately do not defend against

Stated so nobody assumes a guarantee that is not there:

- **An attacker with administrator rights.** They can rewrite the binary, the ACLs, and the OS.
- **A user who deliberately approves a destructive action.** The gate ensures *informed* action, not *wise* action. It is not a nanny.
- **Existing malware running as the user** (A2), against `T2` and `T1`. We can detect and report; we cannot prevent. The honest claim is **"your approval is required"**, not "this machine is clean".
- **The model being wrong.** The gate prevents unauthorised action, not bad judgement.

---

## 7. The one-sentence posture

> **Capybaras guarantees that a destructive action does not run without a human agreeing — and it must never claim more than that.**
