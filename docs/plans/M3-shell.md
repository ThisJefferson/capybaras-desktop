# M3 — The Tauri shell

**The next phase.** Phase 0, 1 and 2 are complete. This plan covers M3 only; M4 (the approval interface) gets its own plan once this exits.

Compiled 2026-09-27. Corrections made to earlier milestone numbering are noted in §7.

---

## Objective

**A double-clickable Windows app that supervises a Node sidecar with correct process lifecycle — carrying in the two rules the spikes bought us.**

Not the Gateway integration. Not the interface. The smallest thing that proves the shell can hold a child process safely, on this platform, with the rules already measured rather than assumed.

**M3 acceptance (from `EXECUTION-PLAN.md` §7):** double-click launches a working agent on a clean user profile.

---

## What the spikes hand to this phase

Two things are already measured, and this phase must implement both or it invalidates them:

| From | Rule | Why it exists |
|---|---|---|
| Spike 001 | Supervise the sidecar with a **Windows Job Object + `KILL_ON_JOB_CLOSE`** | The mechanism that would have answered the spike is the mechanism to build with. Proven in this stack. |
| Spike 001 | **Graceful-stop handshake before any force** | `child.kill()` is `TerminateProcess` on Windows — no `SIGTERM` handler runs. A Gateway owning sessions and sockets must be asked to stop. |
| Spike 002 | **Never resolve paths from `cwd`** | The packaged app launched with `cwd = C:\Windows\system32`. |
| Spike 002 | **Decide the state directory deliberately** | MSIX silently virtualizes AppData writes; the process cannot tell. |

And one piece of debt this phase pays off:

| Open item | From | Action |
|---|---|---|
| Non-orphaning was never measured honestly | Spike 001 | Re-measure **from outside this agent's own process tree** (§6) |
| `app dir writable` reading not representative | Spike 002 | Re-take against a real install at packaging (M6) |

---

## Steps, in order

Each step ends with something runnable. No step ends with "progress."

### Step 1 — Toolchain
Install the Tauri CLI. Rust 1.98.1 and the MSVC build tools are already present (Phase 0), and WebView2 is installed, so this is a user-scope install with no admin.

**Exit:** `cargo tauri --version` returns a v2 version.

### Step 2 — Repo layout for the app
Decide and document where the desktop app lives relative to the existing TypeScript library. The classifier and policy layer stay where they are and are **imported**, not duplicated — the sidecar runs them.

**Exit:** written layout decision; `cargo tauri dev` opens a window.

### Step 3 — Path discipline (spike 002, rule 1)
One module owns every path lookup, resolved from the executable location — never from `cwd`.

**Exit:** a test launches the app with `cwd` forced to `C:\Windows\system32` and it still finds its resources. That is the exact condition spike 002 produced, turned into a regression test.

### Step 4 — State directory (spike 002, rule 2)
Choose deliberately, and document the choice. **Open question that needs measuring, not asserting:** whether a chosen user-visible path survives MSIX virtualization, or whether only the package-private store is reliable. Known-folders redirection is not documented clearly enough for this to be settled by reading.

**Exit:** decision recorded in `DECISIONS.md`; the app writes state to the chosen location and **shows the user where that is**.

### Step 5 — Sidecar supervision
Implement: spawn the Node sidecar, assign it to a Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, request a graceful stop over IPC with a bounded wait, force only after the wait expires, and shut down cleanly on app exit.

**Exit:** the sidecar's `SIGTERM`-equivalent handler **actually runs** on quit — which spike 001 showed does not happen with a plain kill.

### Step 6 — Verify non-orphaning, honestly this time
Spike 001 could not measure this because the agent's own runtime anchors its descendants with a Job Object, so harness death and product death were indistinguishable. Launch the shell from **outside** that tree — a Scheduled Task, or a detached process — kill it, and count survivors.

**Exit:** a measured result written down, whatever it says.

### Step 7 — Single instance, health, tray
Single-instance lock, a health check on the sidecar, and a tray icon wired to the four brand states.

**Exit:** a second launch focuses the first rather than starting a second agent; the tray reflects sidecar state.

---

## What this phase needs from you

**Nothing blocking.** The Tauri CLI is a user-scope install. The open handoffs stay where they are: OpenRouter credit (Phase 5), signing identity (Phase 7), Brazilian copy review (before 1.0), Gateway restart (owner-only).

The one thing I may ask for later: a **clean-profile test at M6**, which is the first milestone the plan says needs a second machine.

---

## Risks

| Risk | Severity | Mitigation |
|---|---|---|
| **Job Object semantics interact with the agent's own job** — my supervisor may inherit a parent job, changing close behaviour | **High** | Step 6 measures from outside the tree. Do not trust an in-tree measurement. |
| Rust build times stall the loop | Medium | Background builds; keep doc work moving meanwhile |
| MSIX virtualization of the chosen state path is worse than expected | Medium | Step 4 measures before committing the layout |
| Scope creep into Gateway integration | Medium | M3 stops at "a supervised child process runs and dies correctly" |

---

## Parallel streams

Streams C (brand), D (docs), E (packaging research) all landed in Phase 2. Two new streams are worth opening alongside M3, both genuinely independent:

- **Approval-card interface design** — the hero component for M4. Design work, not code, and it can run while I build the shell.
- **Bayesian threshold justification** — see `DECISIONS.md` D14. A short document stating the priors and the asymmetric loss function behind each default threshold in the classifier. This is where Bayesian reasoning genuinely helps: justifying *why* a threshold is conservative, without putting a probability anywhere near the safety decision.

---

## §7 — Milestone numbering correction

`EXECUTION-PLAN.md` §7 numbers the milestones: **M2 = spike verdict, M3 = shell runs.** Previous status notes in this repo called the shell "M2." That was wrong, and it is exactly the kind of drift the artifact-driven method exists to catch.

- **M0** Unblocked + scaffolded — done
- **M1** Classifier complete — done, 141 tests
- **M2** Spike verdict — **done**: spike 001 PARTIAL, spike 002 VALIDATED
- **M3** Shell runs — **this plan**

---

## The one-paragraph version

**Build the smallest Tauri app that supervises a Node sidecar correctly on Windows: paths from the executable, a deliberate state directory, a Job Object that guarantees no orphans, and a graceful stop before any force.** Then measure non-orphaning from outside my own process tree, because that is the one thing I could not honestly measure last time. Nothing here is a demo — every step is either a lifecycle guarantee or a regression test for a rule the spikes already paid for.
