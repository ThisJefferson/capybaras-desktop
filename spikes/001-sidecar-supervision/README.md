# Spike 001 — Sidecar supervision on Windows

## Verdict: PARTIAL

**Question:** when a Tauri supervisor spawns a Node sidecar, and the supervisor dies — does the sidecar orphan? And what mechanism prevents it?

**Why it matters:** the whole architecture is a bundled Node Gateway running as a child process of the desktop app. If the app crashes and leaves the Gateway alive, we have a headless agent still holding credentials and network access, with no interface to stop it. That is a security bug, not an inconvenience.

---

## What was built

Throwaway supervisor + sidecar (`supervisor.mjs`, `sidecar.mjs`, `run-spike.ps1`, `run-spike2.ps1`). Three scenarios: clean shutdown, supervisor force-killed, and supervisor force-killed with the sidecar watching its parent.

## Evidence

**Scenario: clean shutdown.** The supervisor's `child.kill()` worked — the sidecar terminated and the supervisor exited. But note the sidecar's `SIGTERM` handler never ran and never logged. On Windows, `child.kill()` calls `TerminateProcess`: it is a hard stop, not a request.

**Scenario: supervisor force-killed (no pipe).** Sidecar was alive before; **not alive 4 s after**. Its log shows heartbeats up to `+2831ms`, then nothing.

**Scenario: supervisor force-killed with parent-watch.** Same outcome — and critically, the watcher's own `PARENT GONE -> exiting by design` line never appeared. The sidecar died *before* its 200 ms watcher could observe the parent's death. Something external killed it.

## What worked

- Clean, deliberate shutdown of the sidecar by the supervisor.
- The 400 ms heartbeat + file-based logging gave usable, timestamped evidence.

## What failed or surprised us

**1. The first harness was confounded and produced contradictory results.** The sidecar's stdout was piped to the supervisor, so when the supervisor died the pipe broke and the sidecar likely died of a write error. The results table said "cleaned up" while a process sweep claimed survivors. **That was bad instrumentation, not a finding.** Rebuilt with `stdio: 'ignore'` and file logging to remove the confound.

**2. Even with the confound removed, the test cannot answer its own question.** The sidecar still dies with the supervisor — but this environment *enforces* process-tree death. The OpenClaw runtime that hosts this work runs a process literally named `service-child-windows-job-anchor.js`, and Windows Job Objects with `KILL_ON_JOB_CLOSE` terminate every descendant when the anchor goes away. **So I cannot distinguish "the product would orphan" from "the harness killed it."** The measurement is contaminated by the instrument.

**3. `child.kill()` is not a graceful shutdown.** No `SIGTERM` handler runs on Windows. For a Gateway that must flush state, close sockets, or write out a session, this matters.

**4. The MSIX half of the question is blocked.** The unverified risk inherited from the packaging research was whether a Node sidecar behaves correctly inside an MSIX *full-trust* package. It cannot be tested here:
- **Developer Mode is off** (`AllowDevelopmentWithoutDevLicense` is unset), so a loose unsigned MSIX cannot be installed.
- Installing a signed package requires a certificate in the trusted store, **which requires elevation** — and elevation is not available from this channel.

So the highest-risk unknown remains unknown, for an environmental reason rather than a technical one.

## Recommendation

**Ship the job-object approach, and stop relying on hope.**

1. **Use a Windows Job Object with `KILL_ON_JOB_CLOSE` in the Tauri supervisor, deliberately.** The mechanism that would have answered this spike is the mechanism we should build with. OpenClaw's own runtime already anchors its children this way — so this is a proven pattern in this exact stack, not an invention.
2. **Add a graceful shutdown handshake.** Ask the Gateway to stop over IPC, wait a bounded time, *then* force. `TerminateProcess` alone risks a half-written state on every quit.
3. **Re-run the orphaning test from outside the harness** — launched by a Scheduled Task or a detached process — during Phase 3, so the instrument stops contaminating the measurement.
4. **Unblock the MSIX test.** It needs either Developer Mode enabled or an elevated shell, once. That is a user action, and it gates the packaging phase.

## Next production step

Build the Tauri shell with a job-object-anchored sidecar and the graceful-stop handshake. Keep the spike code out of production — it is throwaway, as the method requires.

---

## Raw results

```
crash        supervisor=13420  sidecar=27348  alive before=True  alive after=False  -> died with supervisor
crash-watch  supervisor=12700  sidecar=4212   alive before=True  alive after=False  -> died with supervisor
             (parent-watch never logged its exit line)
```

Sidecar log, `crash` scenario, last lines:

```
+   818ms  beat 2 (parent alive: true)
+  1219ms  beat 3 (parent alive: true)
+  1623ms  beat 4 (parent alive: true)
+  2025ms  beat 5 (parent alive: true)
+  2426ms  beat 6 (parent alive: true)
+  2831ms  beat 7 (parent alive: true)
```
